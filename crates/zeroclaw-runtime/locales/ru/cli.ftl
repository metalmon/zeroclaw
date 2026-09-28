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
cli-doctor-degraded-section = раздел конфигурации `{$path}` некорректен и был сброшен к значениям по умолчанию; значения в этом разделе НЕ действуют. Выполните `voltd config migrate`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-doctor-degraded-security = КРИТИЧНЫЙ ДЛЯ БЕЗОПАСНОСТИ раздел конфигурации `{$path}` некорректен и был сброшен к значению по умолчанию, чтобы демон смог запуститься; текущая защита может быть СЛАБЕЕ задуманной. Выполните `voltd config migrate`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-doctor-systemd-linger-disabled = задержка сессии systemd (lingering) отключена; пользовательская служба может остановиться после выхода из системы. Включите командой: loginctl enable-linger {$user}
cli-doctor-systemd-linger-enabled = задержка сессии systemd (lingering) включена
cli-doctor-systemd-linger-unknown = не удалось проверить задержку сессии systemd (lingering) через loginctl
cli-doctor-web-dist-dir-expansion-warning = gateway.web_dist_dir = "{$path}" — {$reason}; gateway.web_dist_dir читается буквально, поэтому раскройте значение самостоятельно (например, абсолютный путь)
cli-doctor-verifiable-intent-tool-withheld = verifiable_intent.enabled включен, но инструмент vi_verify скрыт из видимого модели реестра, пока не появится проверяющий цепочку учетных данных. Включение раздела не включает проверку учетных данных при коммерческих вызовах инструментов. Пути библиотек выпуска и проверки не затронуты.
cli-doctor-memory-semantic-search-without-embedder = memory.search_mode = "{$search_mode}" при памяти sqlite, но эффективный провайдер эмбеддингов не настроен; векторный поиск пропускается, и извлечение работает только по ключевым словам. Настройте memory.embedding_provider или корректный маршрут memory.embedding_model, либо задайте memory.search_mode = "bm25".

# --- ошибки HTTP шлюза: сопряжение / администрирование ---
gateway-paircode-generated = Код сопряжения создан
gateway-pairing-unavailable = Сопряжение отключено или недоступно
gateway-pairing-too-many-requests = Слишком много запросов сопряжения. Повторите позже.
gateway-pairing-too-many-auth = Слишком много попыток авторизации. Повторите через {$secs} с.
gateway-pairing-success = Сопряжение выполнено
gateway-pairing-invalid-code = Неверный или просроченный код сопряжения
gateway-pairing-locked-out = Слишком много попыток. Блокировка на {$secs} с.
gateway-paircode-revoked = Старый токен отозван. Используйте этот код, чтобы заново сопрячь устройство.
gateway-paircode-revoked-pending = Старый токен отозван. Код сопряжения уже ожидает; используйте его или повторите после того, как он истечет.
gateway-admin-localhost-only = Административные эндпоинты доступны только с localhost
gateway-reload-initiated = Перезагрузка демона запущена
gateway-paircode-use-onetime = Используйте этот одноразовый код для сопряжения
gateway-paircode-none-available = Сопряжение активно, но новый код недоступен (уже сопряжено или код истек)
gateway-pairing-disabled = Сопряжение отключено для этого шлюза
gateway-paircode-mint-forbidden = Создание кода сопряжения требует localhost или админ-токена
gateway-pair-code-invalid = Неверный код сопряжения
gateway-shutdown-initiated = Остановка шлюза запущена
gateway-reload-remote-disabled = Удаленная перезагрузка администратора отключена. Вызовите с localhost или установите gateway.allow_remote_admin = true (при включенном сопряжении, затем выполните сопряжение), чтобы разрешить аутентифицированную удаленную перезагрузку.
gateway-reload-requires-pairing = Удаленная перезагрузка администратора требует сопряжения. gateway.allow_remote_admin включен, но gateway.require_pairing выключен, поэтому удаленные клиенты не могут быть аутентифицированы. Включите require_pairing или вызовите /admin/reload с localhost.
gateway-reload-no-supervisor = нет супервизора демона — работает как автономный шлюз. Перезапустите процесс, чтобы применить изменения конфигурации.
gateway-device-registry-disabled = Реестр устройств отключен; невозможно ротировать отдельное устройство.
gateway-device-not-found = Устройство «{$device}» не найдено; ничего не отозвано.
gateway-device-registry-error = Ошибка реестра устройств: {$err}
gateway-token-persist-failed = Токен отозван в памяти, но сохранение конфигурации не удалось: {$err}
gateway-paircode-revoked-device = Токен устройства «{$device}» отозван.
gateway-principal-not-configured = субъект «{$principal}» не настроен; сначала подключите его (используйте несвязанный код сопряжения, затем назначьте ему профиль в разделе «Роли») перед созданием предсвязанного кода для него
gateway-paircode-new-revoked-principal = {$revoked} Используйте этот одноразовый код для повторного сопряжения, помечено для субъекта «{$principal}».
gateway-paircode-new-revoked = {$revoked} Используйте этот одноразовый код для повторного сопряжения.
gateway-paircode-new-principal = Код сопряжения создан, помечен для субъекта «{$principal}» — используйте этот одноразовый код для сопряжения
gateway-paircode-new = Код сопряжения создан — используйте этот одноразовый код для сопряжения
gateway-pairing-failed-token-dropped = Сопряжение не удалось; токен в процессе не сохранен.
gateway-token-persistence-error = Ошибка сохранения токена: {$err}
gateway-save-token-hint = Сохраните этот токен — используйте его как Authorization: Bearer <token>
gateway-too-many-failed-attempts = Слишком много неудачных попыток. Повторите через {$secs} с.
gateway-tokens-registry-clear-failed = Токены отозваны в памяти, но очистка реестра устройств не удалась: {$err}
gateway-tokens-persist-failed = Токены отозваны в памяти, но сохранение конфигурации не удалось: {$err}
gateway-device-revoked = Устройство отозвано, токен доступа аннулирован
gateway-capabilities-updated = Возможности обновлены
gateway-old-token-revoked-disabled = Старый токен отозван. Сопряжение отключено; невозможно выдать новый код.
# HTTP-ошибки webhook / СОП / интеграций
gateway-webhook-too-many-requests = Слишком много запросов webhook. Повторите позже.
gateway-webhook-unauthorized-pair = Не авторизовано — сначала выполните сопряжение через POST /pair, затем отправьте Authorization: Bearer <token>
gateway-webhook-unauthorized-secret = Не авторизовано — неверный или отсутствующий заголовок X-Webhook-Secret
gateway-idempotency-key-reserved = Предыдущий запрос уже зарезервировал этот ключ идемпотентности; новая отправка не запущена
gateway-sop-webhook-needs-credential = Отправка webhook СОП требует настроенных учетных данных: задайте `gateway.require_pairing = true` и аутентифицируйтесь через `Authorization: Bearer <paired-token>` (сначала выполните сопряжение через POST /pair), либо задайте `gateway.webhook_secret` и отправляйте X-Webhook-Secret.
gateway-invalid-json-body = Неверное тело JSON. Ожидается: {"{"}"message": "..."{"}"}
gateway-llm-request-failed = Запрос к LLM не удался
gateway-invalid-json-payload = Неверные данные JSON
gateway-gmail-push-not-configured = Gmail push не настроен
gateway-request-body-too-large = Тело запроса слишком велико
gateway-unauthorized = Не авторизовано
gateway-invalid-pubsub-envelope = Неверный конверт Pub/Sub
gateway-missing-hub-challenge = Отсутствует hub.challenge
gateway-forbidden = Доступ запрещен
gateway-device-not-found-plain = Устройство не найдено
gateway-missing-bearer-token = Отсутствует токен доступа
gateway-device-not-found-for-token = Устройство для этого токена не найдено
