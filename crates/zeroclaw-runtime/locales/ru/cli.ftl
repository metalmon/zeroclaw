# Russian CLI strings for ZeroClaw.
# Keys not present here fall back to the English catalog in ../en/cli.ftl.

# --- doctor diagnostics ---
cli-doctor-config-file = Файл конфигурации: {$path}
cli-doctor-config-file-missing = Файл конфигурации не найден: {$path}
cli-doctor-provider-invalid = model_provider "{$label}" недействителен: {$reason}
cli-doctor-provider-valid = model_provider "{$label}" корректен
cli-doctor-api-key-configured = {$label}: API-ключ настроен
cli-doctor-provider-api-key-missing = {$label}: api_key не задан (возможно, используются переменные окружения или значения model_provider по умолчанию)
cli-doctor-provider-model = {$label}: модель: {$model}
cli-doctor-provider-model-missing = {$label}: модель не настроена
cli-doctor-no-model-providers = провайдеры моделей не настроены
cli-doctor-provider-temperature-ok = {$label}: temperature {$temperature} (допустимый диапазон 0.0–2.0)
cli-doctor-provider-temperature-out-of-range = {$label}: temperature {$temperature} вне диапазона (ожидается 0.0–2.0)
cli-doctor-provider-temperature-unset = {$label}: temperature не задан (значение провайдера по умолчанию)
cli-doctor-provider-api-key-not-registered = {$label}: api_key не задан — этот провайдер НЕ будет зарегистрирован (не мягкий откат); задайте `[{$label}].api_key` или удалите запись
cli-doctor-gateway-port = порт gateway: {$port}
cli-doctor-gateway-port-invalid = порт gateway равен 0 (недопустимо)
cli-doctor-model-route-empty-hint = маршрут модели с пустым hint
cli-doctor-model-route-invalid-provider = маршрут модели "{$hint}" использует недействительный model_provider "{$provider}": {$reason}
cli-doctor-model-route-empty-model = маршрут модели "{$hint}" имеет пустое поле model
cli-doctor-embedding-route-empty-hint = маршрут эмбеддингов с пустым hint
cli-doctor-embedding-route-invalid-provider = маршрут эмбеддингов "{$hint}" использует недействительный model_provider "{$provider}": {$reason}
cli-doctor-embedding-route-empty-model = маршрут эмбеддингов "{$hint}" имеет пустое поле model
cli-doctor-embedding-route-invalid-dimensions = маршрут эмбеддингов "{$hint}" имеет недопустимое dimensions=0
cli-doctor-embedding-hint-no-route = memory.embedding_model использует hint "{$hint}", но нет соответствующей записи [[embedding_routes]]
cli-doctor-channel-present = настроен хотя бы один канал
cli-doctor-no-channels = нет настроенных каналов — выполните `zeroclaw quickstart`, чтобы настроить канал
cli-doctor-telegram-bot-token-unset = channels.telegram.{$alias}.bot_token не задан, но канал включен — канал не сможет подключиться, пока не задан токен бота
cli-doctor-discord-bot-token-unset = channels.discord.{$alias}.bot_token не задан, но канал включен — канал не сможет подключиться, пока не задан токен бота
cli-doctor-agent-invalid-provider = агент "{$name}" использует недействительный model_provider "{$provider}": {$reason}
cli-doctor-config-warning = {$message} (в {$path})
cli-doctor-workspace-exists = каталог существует: {$path}
cli-doctor-workspace-missing = каталог отсутствует: {$path}
cli-doctor-workspace-writable = каталог доступен для записи
cli-doctor-workspace-write-probe-failed = проверка записи в каталог не удалась: {$error}
cli-doctor-workspace-not-writable = каталог недоступен для записи: {$error}
cli-doctor-disk-space-ok = дисковое пространство: доступно {$available} МБ
cli-doctor-disk-space-low = мало места на диске: доступно всего {$available} МБ
cli-doctor-agent-file-present = [{$alias}] {$name} присутствует
cli-doctor-agent-file-missing = [{$alias}] {$name} не найден (необязательно)
cli-doctor-daemon-state-missing = файл состояния не найден: {$path} — запущен ли демон?
cli-doctor-daemon-state-read-failed = не удается прочитать файл состояния: {$error}
cli-doctor-daemon-state-invalid-json = некорректный JSON состояния: {$error}
cli-doctor-daemon-heartbeat-fresh = heartbeat актуален ({$age}с назад)
cli-doctor-daemon-heartbeat-stale = heartbeat устарел ({$age}с назад)
cli-doctor-daemon-timestamp-invalid = некорректная метка времени демона: {$timestamp}
cli-doctor-scheduler-healthy = планировщик исправен (последний успех {$age}с назад)
cli-doctor-scheduler-unhealthy = планировщик неисправен (ok={$ok}, age={$age}с)
cli-doctor-scheduler-not-tracked = компонент планировщика еще не отслеживается
cli-doctor-channel-fresh = {$name} актуален ({$age}с назад)
cli-doctor-channel-stale = {$name} устарел (ok={$ok}, age={$age}с)
cli-doctor-no-channel-components = компоненты каналов еще не отслеживаются
cli-doctor-channels-stale-summary = каналов: {$count}, устарело: {$stale}
cli-doctor-shell-set = оболочка: {$shell}
cli-doctor-shell-unset = не заданы ни $SHELL, ни %ComSpec%
cli-doctor-home-set = переменная домашнего каталога задана
cli-doctor-home-unset = не заданы ни $HOME, ни $USERPROFILE
cli-doctor-no-cli-tools = CLI-инструменты в PATH не найдены
cli-doctor-cli-tool-entry = {$name} ({$category}) — {$version}
cli-doctor-cli-tools-count = обнаружено CLI-инструментов: {$count}
cli-doctor-command-version = {$cmd}: {$version}
cli-doctor-command-nonzero = {$cmd} найден, но вернул ненулевой код
cli-doctor-command-not-found = {$cmd} не найден в PATH

# --- doctor diagnostics (pre-existing keys, web-visible) ---
cli-doctor-context-window-ok = {$provider_ref}: окно контекста: {$context_window} токенов
cli-doctor-context-window-unset = {$provider_ref}: context_window не задан — при выборе будет использован запасной лимит {$fallback} токенов; вероятно, намного ниже реального лимита модели; задайте context_window в этом профиле
cli-doctor-context-window-zero = {$provider_ref}: context_window равен 0 (недопустимо; задайте реальный лимит контекста модели)
cli-doctor-degraded-section = раздел конфигурации `{$path}` некорректен и был сброшен к значениям по умолчанию; значения в этом разделе НЕ действуют. Выполните `zeroclaw config migrate`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-doctor-degraded-security = КРИТИЧНЫЙ ДЛЯ БЕЗОПАСНОСТИ раздел конфигурации `{$path}` некорректен и был сброшен к значению по умолчанию, чтобы демон смог запуститься; текущая защита может быть СЛАБЕЕ задуманной. Выполните `zeroclaw config migrate`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-doctor-systemd-linger-disabled = задержка сессии systemd (lingering) отключена; пользовательская служба может остановиться после выхода из системы. Включите командой: loginctl enable-linger {$user}
cli-doctor-systemd-linger-enabled = задержка сессии systemd (lingering) включена
cli-doctor-systemd-linger-unknown = не удалось проверить задержку сессии systemd (lingering) через loginctl
cli-doctor-web-dist-dir-expansion-warning = gateway.web_dist_dir = "{$path}" — {$reason}; gateway.web_dist_dir читается буквально, поэтому раскройте значение самостоятельно (например, абсолютный путь)
cli-doctor-verifiable-intent-tool-withheld = verifiable_intent.enabled включен, но инструмент vi_verify скрыт из видимого модели реестра, пока не появится проверяющий цепочку учетных данных. Включение раздела не включает проверку учетных данных при коммерческих вызовах инструментов. Пути библиотек выпуска и проверки не затронуты.
