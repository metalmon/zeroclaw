# Russian CLI strings for Вольт.
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

# --- Gateway API errors (RU localization, batch 2) ---
gateway-webauthn-disabled = WebAuthn не включен
gateway-webauthn-no-pending-registration = Нет ожидающей регистрации для этого challenge
gateway-webauthn-no-pending-authentication = Нет ожидающей аутентификации для этого challenge
gateway-canvas-not-found = Canvas «{$id}» не найден
gateway-canvas-invalid-content-type = Недопустимый content_type «{$content_type}». Разрешено: {$allowed}
gateway-canvas-content-too-large = Содержимое превышает максимальный размер {$max} байт
gateway-canvas-max-count-reached = Достигнуто максимальное число canvas. Сначала очистите неиспользуемые canvas.
gateway-canvas-max-count-reached-ws = Достигнуто максимальное число canvas
gateway-canvas-ws-unauthorized = Не авторизовано — укажите заголовок Authorization или Sec-WebSocket-Protocol bearer
gateway-plugins-unauthorized = Не авторизовано
gateway-static-invalid-path = Недопустимый путь
gateway-static-not-found = Не найдено
gateway-static-no-backend-route = Ни один backend-маршрут не совпал с этим путем.
gateway-static-dashboard-unavailable = Веб-панель недоступна. Переустановите с помощью поддерживаемого установщика, чтобы панель была собрана и размещена там, где ее ищет gateway: `./install.sh --source` в Linux/macOS или `setup.bat` в Windows. API-эндпоинты демона остаются доступными независимо от панели.
gateway-sop-disabled = Подсистема SOP не включена
gateway-sop-lock-poisoned = Блокировка движка SOP повреждена
gateway-sop-remote-approval-disabled = Удаленное подтверждение SOP отключено. Обратитесь с localhost или задайте gateway.allow_remote_admin = true с включенным сопряжением, затем выполните сопряжение.
gateway-sop-resolve-failed = не удалось разрешить: {$err}
gateway-sop-payload-invalid-json = полезная нагрузка не является корректным JSON
gateway-sop-no-manual-trigger = SOP «{$name}» не имеет подходящего ручного триггера
gateway-sop-invalid-decision = решение не является допустимым решением о подтверждении: {$err}
gateway-sop-run-not-found = Запуск {$run_id} не найден
gateway-sop-run-belongs-to-other = запуск «{$run_id}» относится к SOP «{$sop}», а не «{$name}»
gateway-sop-run-not-waiting-approval = Запуск {$run_id} не ожидает подтверждения
gateway-sop-self-approval-forbidden = approval_mode запрещает этому субъекту снимать блокировку
gateway-sop-not-authorized-group = нет доступа: требуется группа «{$group}»
gateway-sop-policy-not-configured = политика подтверждения «{$name}» не настроена (блокировка оставлена в ожидании)
gateway-sop-run-not-waiting-or-checkpoint = Запуск {$run_id} не ожидает подтверждения и не приостановлен на контрольной точке
gateway-sop-remote-cancel-requires-pairing = Удаленная отмена SOP требует сопряжения шлюза. Сначала включите gateway.require_pairing и выполните сопряжение или обратитесь с localhost.
gateway-sop-invalid-cancel-body = некорректное тело запроса отмены: {$err}
gateway-sop-run-disappeared = запуск исчез после перехода к отмене
gateway-sop-cancel-not-persisted = отмену не удалось надежно сохранить; запуск остается активным - повторите попытку
gateway-sop-body-name-mismatch = имя в теле «{$body}» не совпадает с именем в URL «{$url}»
gateway-sop-load-error = SOP «{$name}»: {$err}
gateway-cron-tz-nonempty = tz должен быть непустой таймзоной IANA; используйте clear_tz=true, чтобы очистить его
gateway-cron-tz-xor = Укажите либо tz, либо clear_tz=true, но не оба сразу
gateway-cron-schedule-invalid = Неверное расписание cron: {$err}
gateway-cron-list-failed = Не удалось получить список заданий cron: {$err}
gateway-misc-unknown-agent = Неизвестный агент {$agent} (нет настроенной записи [agents.{$alias}])
gateway-cron-add-failed = Не удалось добавить задание cron: {$err}
gateway-cron-shellfmt-agent = shell_output_format неприменим к заданиям агента; выполнение агента его игнорирует
gateway-cron-missing-prompt = Отсутствует 'prompt' для задания агента
gateway-cron-missing-command = Отсутствует 'command' для shell-задания
gateway-cron-not-found = Задание cron не найдено: {$err}
gateway-cron-runs-list-failed = Не удалось получить список запусков cron: {$err}
gateway-cron-shellfmt-declarative = shell_output_format для декларативного задания '{$id}' задается через cron.{$id}.shell_output_format в config.toml, а не через API; столбец в БД не читается для декларативных заданий, и этот PATCH не даст эффекта
gateway-cron-tz-schedule-only = tz можно обновлять только для расписаний cron
gateway-cron-update-failed = Не удалось обновить задание cron: {$err}
gateway-cron-remove-failed = Не удалось удалить задание cron: {$err}
gateway-cfg-save-failed = Не удалось сохранить конфигурацию: {$err}
gateway-mem-build-failed = Не удалось построить память для агента: {$err}
gateway-mem-recall-failed = Не удалось выполнить поиск в памяти: {$err}
gateway-mem-list-failed = Не удалось получить список записей памяти: {$err}
gateway-mem-store-failed = Не удалось сохранить запись в память: {$err}
gateway-mem-forget-failed = Не удалось удалить запись из памяти: {$err}
gateway-misc-cost-summary-failed = Не удалось получить сводку расходов: {$err}
gateway-channel-unknown = неизвестный канал {$channel} — используйте составное имя из GET /api/channels
gateway-channel-no-relink = канал типа {$channel} не поддерживает операцию перепривязки (он не использует сессии QR-сопряжения) либо эта возможность не включена в сборку данного бинарного файла; ничего не изменено
gateway-channel-relink-failed = не удалось очистить сохраненные данные входа: {$err}
gateway-session-content-required = требуется content
gateway-session-persistence-disabled = Сохранение сессий отключено
gateway-session-not-found = Сессия не найдена
gateway-session-queue-full = Очередь сессии переполнена
gateway-session-queue-timeout = Истекло время ожидания очереди сессии
gateway-session-append-failed = Не удалось добавить сообщение в сессию: {$err}
gateway-session-delete-failed = Не удалось удалить сессию: {$err}
gateway-session-name-required = требуется name
gateway-session-rename-failed = Не удалось переименовать сессию: {$err}
gateway-session-state-failed = Не удалось получить состояние сессии: {$err}
