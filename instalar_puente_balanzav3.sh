#!/bin/bash

set -euo pipefail

echo "🔧 Compilando puente_balanzav3 con cargo..."
cargo build

# Rutas
BIN_ORIG="target/debug/puente_balanzav3"
BIN_DEST="/usr/bin/puente_balanzav3"
CONFIG_SOURCE="config.toml"
CONFIG_DEST_DIR="/etc/puente_balanzav3"
SERVICE_FILE="puente_balanzav3.service"
SYSTEMD_PATH="/etc/systemd/system/puente_balanzav3.service"

# 1. Copiar binario a /usr/bin
echo "📄 Copiando binario a $BIN_DEST"
sudo cp "$BIN_ORIG" "$BIN_DEST"
sudo chmod +x "$BIN_DEST"

# 2. Crear directorio de configuración
echo "📁 Creando directorio de configuración: $CONFIG_DEST_DIR"
sudo mkdir -p "$CONFIG_DEST_DIR"

# 3. Copiar archivo de configuración
if [ -f "$CONFIG_SOURCE" ]; then
    echo "⚙️ Copiando archivo de configuración $CONFIG_SOURCE a $CONFIG_DEST_DIR"
    sudo cp "$CONFIG_SOURCE" "$CONFIG_DEST_DIR/"
else
    echo "⚠️ Advertencia: No se encontró $CONFIG_SOURCE en el directorio actual. El servicio puede fallar si lo necesita."
fi

# 4. Copiar archivo .service a systemd
echo "⚙️ Instalando servicio systemd en $SYSTEMD_PATH"
sudo cp "$SERVICE_FILE" "$SYSTEMD_PATH"

# 5. Deshabilitar servicios anteriores tipo puente_balanza*
echo "🛑 Deshabilitando servicios anteriores tipo puente_balanza*..."
OLD_SERVICES=$(systemctl list-unit-files | grep '^puente_balanza' | awk '{print $1}' || true)
for svc in $OLD_SERVICES; do
    echo "  ➤ Deshabilitando $svc"
    sudo systemctl disable "$svc" || true
done

# 6. Recargar systemd
echo "🔄 Recargando systemd"
sudo systemctl daemon-reexec
sudo systemctl daemon-reload

# 7. Habilitar e iniciar nuevo servicio
echo "✅ Habilitando puente_balanzav3.service"
sudo systemctl enable puente_balanzav3.service

echo "🚀 Iniciando puente_balanzav3.service"
sudo systemctl restart puente_balanzav3.service

# 8. Mostrar log y preguntar
echo "📋 Mostrando últimas 10 líneas del journal:"
sudo journalctl -u puente_balanzav3.service -n 10 --no-pager

read -rp "¿Deseas seguir viendo el log en tiempo real? [s/N]: " RESPUESTA
case "$RESPUESTA" in
    [sS]|[sS][iI])
        echo "📡 Mostrando logs en tiempo real. Presiona Ctrl+C para salir."
        sudo journalctl -fu puente_balanzav3.service
        ;;
    *)
        echo "✅ Instalación finalizada. Puedes ver los logs con: sudo journalctl -fu puente_balanzav3.service"
        ;;
esac

