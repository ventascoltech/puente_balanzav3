use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use flume::Sender;

use anyhow::{Context, Result};
use log::{info, warn};

use crate::cache::SharedCache;
use crate::config::RuntimeConfig;
use crate::command::Comando;
use crate::serial_utils::sanitize_log_data;

/// Tipos de verificación sobre la caché
enum CacheCheck {
    ValidoDesdePasado(Duration),
    PosteriorA(Instant, Duration),
}

/// Inicia el servidor TCP y acepta conexiones entrantes.
pub fn start_tcp_server(runtime_config: &RuntimeConfig, cache: SharedCache) {
    if let Err(e) = run_server(runtime_config, cache) {
        warn!("❌ Error en el servidor TCP: {:?}", e);
    }
}

/// Ejecuta el bucle principal del servidor TCP.
fn run_server(runtime_config: &RuntimeConfig, cache: SharedCache) -> Result<()> {
    let config_guard = runtime_config.config.read();
    let listener = TcpListener::bind(config_guard.address())
        .context("No se pudo iniciar el servidor TCP")?;

    info!("🟢 Servidor TCP escuchando en {}", config_guard.address());
    drop(config_guard);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let peer = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "desconocido".to_string());
                info!("🔌 Nueva conexión desde {}", peer);

                let cache = cache.clone();
                let config = runtime_config.config.clone();
                let sender = runtime_config.serial_write_sender.clone();

                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, config, sender, cache) {
                        warn!("❌ Error manejando cliente: {:?}", e);
                    }
                });
            }
            Err(e) => warn!("⚠️ Error al aceptar conexión: {}", e),
        }
    }

    Ok(())
}

/// Maneja una conexión con un cliente.
fn handle_client(
    mut stream: TcpStream,
    config: std::sync::Arc<parking_lot::RwLock<crate::config::Config>>,
    sender: Sender<Vec<u8>>,
    cache: SharedCache,
) -> Result<()> {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    let mut buffer = [0u8; 1024];

    loop {
        let bytes_read = match stream.read(&mut buffer) {
            Ok(0) => {
                info!("🔌 Cliente desconectado [{}]", peer);
                break;
            }
            Ok(n) => n,
            Err(e) => {
                warn!("⚠️ Error al leer del cliente [{}]: {}", peer, e);
                break;
            }
        };

        let comando_str = String::from_utf8_lossy(&buffer[..bytes_read]).trim().to_string();
        info!(
            "📥 Comando recibido del cliente [{}]: '{}'",
            peer,
            sanitize_log_data(comando_str.as_bytes())
        );

        match Comando::parse(&comando_str) {
            Some(Comando::Uno) => {
                let config_guard = config.read();
                responder_con_cache(
                    &mut stream,
                    &cache,
                    CacheCheck::ValidoDesdePasado(Duration::from_millis(config_guard.cache_duration_ms)),
                    b"NO DATA\n",
                )?;
            }
            Some(Comando::W) => {
                manejar_comando_w(&mut stream, &config, &sender, &cache, &peer)?;
            }
            None => {
                warn!(
                    "⚠️ Comando no reconocido del cliente [{}]: '{}'",
                    peer,
                    sanitize_log_data(comando_str.as_bytes())
                );
                let _ = stream.write_all(b"Comando invalido\n");
            }
        }
    }

    Ok(())
}

/// Envía al cliente el dato de la caché si cumple con el criterio, o un mensaje alternativo si no lo hace.
fn responder_con_cache(
    stream: &mut TcpStream,
    cache: &SharedCache,
    criterio: CacheCheck,
    no_data_msg: &[u8],
) -> Result<()> {
    let guard = cache.lock();
    let resultado = match criterio {
        CacheCheck::ValidoDesdePasado(duracion) => {
            guard
                .get_raw()
                .filter(|(_, t)| t.elapsed() <= duracion)
                .map(|(data, _)| data)
        }
        CacheCheck::PosteriorA(start, ventana) => {
            guard
                .get_raw()
                .filter(|(_, t)| t >= &start && t <= &(start + ventana))
                .map(|(data, _)| data)
        }
    };

    match resultado {
        Some(data) => {
            stream.write_all(data).context("Error al enviar datos al cliente")?;
            info!(
                "✅ Dato enviado al cliente: {}",
                sanitize_log_data(data)
            );
        }
        None => {
            warn!("⚠️ No se encontró dato válido en caché según el criterio.");
            let _ = stream.write_all(no_data_msg);
        }
    }

    Ok(())
}

/// Maneja el comando 'W' bajo la lógica propuesta.
fn manejar_comando_w(
    stream: &mut TcpStream,
    config: &std::sync::Arc<parking_lot::RwLock<crate::config::Config>>,
    sender: &Sender<Vec<u8>>,
    cache: &SharedCache,
    _peer: &str,
) -> Result<()> {
    let (w_duration_ms, w_response_timeout_ms) = {
        let c = config.read();
        (c.w_duration_ms, c.w_response_timeout_ms)
    };

    let w_duration = Duration::from_millis(w_duration_ms);
    let timeout = Duration::from_millis(w_response_timeout_ms);

    // Paso 1: Intentar usar caché reciente
    {
        let guard = cache.lock();
        if let Some((data, t)) = guard.get_raw() {
            if t.elapsed() <= w_duration {
                stream.write_all(data)?;
                info!(
                    "✅ Dato enviado al cliente: {}",
                    sanitize_log_data(data)
                );
                return Ok(());
            } else {
                warn!("⚠️ No se encontró dato válido en caché según el criterio.");
            }
        } else {
            warn!("⚠️ No se encontró dato válido en caché según el criterio.");
        }
    }

    // Paso 2: Solicitar nuevo dato
    info!("📤 Cache inválida/vencida. Enviando 'W' a la báscula...");
    sender.send(b"W".to_vec()).context("Error enviando 'W' al serial")?;

    let inicio = Instant::now();
    let mut intento = 0;
    let mut logueado = false;
    let max_intentos = (timeout.as_millis() / 50).min(20) as u32;

    while intento < max_intentos {
        {
            let guard = cache.lock();
            if let Some((data, t)) = guard.get_raw() {
                if t >= inicio && t <= inicio + timeout {
                    stream.write_all(data)?;
                    info!(
                        "✅ Dato enviado al cliente: {}",
                        sanitize_log_data(data)
                    );
                    return Ok(());
                }
            }

            if !logueado {
                warn!("⚠️ No se encontró dato válido en caché según el criterio.");
                logueado = true;
            } else if intento == 5 {
                warn!("⚠️ Aún no se recibe un dato válido después de 5 intentos...");
            }
        }

        thread::sleep(Duration::from_millis(50));
        intento += 1;
    }

    warn!("⏱️ Timeout esperando nuevo dato luego de 'W'");
    let _ = stream.write_all(b"W_TIMEOUT\n");
    Ok(())
}

