# Russian CLI strings for Volt.
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
cli-doctor-provider-api-key-not-registered = {$label}: api_key не задан - этот провайдер НЕ будет зарегистрирован (не мягкий откат); задайте `[{$label}].api_key` или удалите запись
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
cli-doctor-no-channels = нет настроенных каналов - выполните `voltd quickstart`, чтобы настроить канал
cli-doctor-telegram-bot-token-unset = channels.telegram.{$alias}.bot_token не задан, но канал включен - канал не сможет подключиться, пока не задан токен бота
cli-doctor-discord-bot-token-unset = channels.discord.{$alias}.bot_token не задан, но канал включен - канал не сможет подключиться, пока не задан токен бота
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
cli-doctor-daemon-state-missing = файл состояния не найден: {$path} - запущен ли демон?
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
cli-doctor-cli-tool-entry = {$name} ({$category}) - {$version}
cli-doctor-cli-tools-count = обнаружено CLI-инструментов: {$count}
cli-doctor-command-version = {$cmd}: {$version}
cli-doctor-command-nonzero = {$cmd} найден, но вернул ненулевой код
cli-doctor-command-not-found = {$cmd} не найден в PATH

# --- doctor diagnostics (pre-existing keys, web-visible) ---
cli-doctor-context-window-ok = {$provider_ref}: окно контекста: {$context_window} токенов
cli-doctor-context-window-unset = {$provider_ref}: context_window не задан - при выборе будет использован запасной лимит {$fallback} токенов; вероятно, намного ниже реального лимита модели; задайте context_window в этом профиле
cli-doctor-context-window-zero = {$provider_ref}: context_window равен 0 (недопустимо; задайте реальный лимит контекста модели)
cli-doctor-degraded-section = раздел конфигурации `{$path}` некорректен и был сброшен к значениям по умолчанию; значения в этом разделе НЕ действуют. Выполните `voltd config migrate`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-doctor-degraded-security = КРИТИЧНЫЙ ДЛЯ БЕЗОПАСНОСТИ раздел конфигурации `{$path}` некорректен и был сброшен к значению по умолчанию, чтобы демон смог запуститься; текущая защита может быть СЛАБЕЕ задуманной. Выполните `voltd config migrate`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-doctor-systemd-linger-disabled = задержка сессии systemd (lingering) отключена; пользовательская служба может остановиться после выхода из системы. Включите командой: loginctl enable-linger {$user}
cli-doctor-systemd-linger-enabled = задержка сессии systemd (lingering) включена
cli-doctor-systemd-linger-unknown = не удалось проверить задержку сессии systemd (lingering) через loginctl
cli-doctor-web-dist-dir-expansion-warning = gateway.web_dist_dir = "{$path}" - {$reason}; gateway.web_dist_dir читается буквально, поэтому раскройте значение самостоятельно (например, абсолютный путь)
cli-doctor-verifiable-intent-tool-withheld = verifiable_intent.enabled включен, но инструмент vi_verify скрыт из видимого модели реестра, пока не появится проверяющий цепочку учетных данных. Включение раздела не включает проверку учетных данных при коммерческих вызовах инструментов. Пути библиотек выпуска и проверки не затронуты.
cli-doctor-memory-semantic-search-without-embedder = memory.search_mode = "{$search_mode}" при памяти sqlite, но эффективный провайдер эмбеддингов не настроен; векторный поиск пропускается, и извлечение работает только по ключевым словам. Настройте memory.embedding_provider или корректный маршрут memory.embedding_model, либо задайте memory.search_mode = "bm25".
cli-doctor-cli-tool-version-unknown = версия неизвестна
cli-doctor-model-probe-count = моделей: {$count}
cli-doctor-provider-invalid-generic = недействительный model_provider
cli-doctor-embedding-provider-supported-values = допустимые значения: none, openai, custom:<url>
cli-doctor-embedding-provider-custom-url-empty = custom model_provider требует непустой URL после 'custom:'
cli-doctor-embedding-provider-custom-url-scheme = URL custom model_provider должен использовать http/https, получено '{$scheme}'
cli-doctor-embedding-provider-custom-url-invalid = некорректный URL custom model_provider: {$error}
cli-doctor-bootstrap-file-truncated = [{$alias}] {$file}: на ходах цикла агента и каналов, которые его подключают, лимит на файл сохраняет {$retained} из {$total} символов ({$discarded} отброшено, до учета бюджета всего промпта). Каждый bootstrap-файл ограничен {$limit} символами; сократите файл.
cli-doctor-bootstrap-file-truncated-compact = [{$alias}] {$file}: на ходах цикла агента и каналов, которые его подключают, лимит на файл сохраняет {$retained} из {$total} символов ({$discarded} отброшено, до учета бюджета всего промпта). compact_context включен для этого агента и ограничивает каждый bootstrap-файл {$limit} символами. Задайте `compact_context = false` в `[runtime_profiles.{$profile}]` или сократите файл.
cli-doctor-bootstrap-file-truncated-compact-no-profile = [{$alias}] {$file}: на ходах цикла агента и каналов, которые его подключают, лимит на файл сохраняет {$retained} из {$total} символов ({$discarded} отброшено, до учета бюджета всего промпта). compact_context включен для этого агента (по умолчанию, профиль выполнения не назначен) и ограничивает каждый bootstrap-файл {$limit} символами. Добавьте `[runtime_profiles.<name>]` с `compact_context = false` и задайте агенту `runtime_profile = "<name>"` или сократите файл.
cli-doctor-cache-write-failed = Не удалось сохранить кэш моделей: {$error}
cli-doctor-codex-auth-ok = Учетные данные OpenAI Codex подключены, и на них ссылается слот провайдера моделей.
cli-doctor-codex-auth-profile-no-slot = Учетные данные OpenAI Codex подключены, но ни один слот провайдера моделей их не использует. Задайте `requires_openai_auth = true` в слоте провайдера OpenAI и укажите на него `model_provider` агента либо выполните `voltd quickstart`.
cli-doctor-codex-auth-slot-no-profile = Слот(ы) OpenAI {$slots} задают `requires_openai_auth = true`, но учетные данные OpenAI Codex не подключены. Выполните `voltd auth login --provider openai-codex`.
cli-doctor-probe-timeout-message = Опрос моделей превысил время ожидания. Некоторые каталоги провайдеров могут быть недоступны. Повторите Диагностику, чтобы обновить.
cli-doctor-security-audit-disabled-drops-certificate-record = security.audit.enabled=false: сертификаты выпускаются и обновляются без записи в аудит. Выполнение команд в любом случае не аудируется, так как ни один рабочий путь не записывает команды инструментов. Оставьте раздел включенным, чтобы сохранить след сертификатов, а если нужна запись о том, что выполнялось, используйте внешний супервизор или логирующую обертку, наблюдающую за процессом Volt, либо учет процессов на уровне ОС.

# --- doctor update-context-windows (CLI output) ---
cli-doctor-ctxwin-already-set = {$provider_ref}: context_window уже задан = {$ctx}
cli-doctor-ctxwin-dry-run = Пробный запуск завершен - изменения не записаны. Запустите без --dry-run, чтобы применить.
cli-doctor-ctxwin-fetch-failed = {$provider_ref}: провайдер не сообщает окно контекста или запрос не удался
cli-doctor-ctxwin-no-model = {$provider_ref}: модель не настроена, пропуск
cli-doctor-ctxwin-none = Обновления не требуются.
cli-doctor-ctxwin-not-found = {$provider_ref}: запись для обновления не найдена
cli-doctor-ctxwin-saved = Сохранено обновлений в config.toml: {$updated}
cli-doctor-ctxwin-set = {$provider_ref}: задан context_window = {$ctx}
cli-doctor-ctxwin-would-set = {$provider_ref}: будет задан context_window = {$ctx} (пробный запуск)
cli-doctor-ctxwin-write-failed = {$provider_ref}: не удалось записать context_window: {$error}

# --- daemon startup banner ---
cli-daemon-gateway-already-running = Шлюз Volt уже запущен на {$host}:{$port}. Демон управляет собственным шлюзом и не запустит второй на том же адресе. Остановите тот шлюз (или укажите демону свободный порт командой `voltd config set gateway.port <port>`), затем запустите демон снова.
cli-daemon-gateway-port-occupied = Адрес шлюза {$host}:{$port} уже занят другим процессом. Освободите порт или укажите демону свободный порт (`voltd config set gateway.port <port>`), затем запустите демон снова.
cli-daemon-started-gateway = Шлюз:     {$url}
cli-daemon-started-pairing = Сопряжение: включено (текущий статус см. в выводе шлюза выше)
cli-daemon-started-socket = Сокет:    {$path}
cli-daemon-started-stop = Ctrl+C или SIGTERM для остановки
cli-daemon-started-title = 🧠 Демон Volt готов
cli-daemon-starting-detail = Подготовка настроенных конечных точек демона
cli-daemon-starting-title = 🧠 Демон Volt запускается…

# --- gateway restart hints ---
cli-gateway-restart-hint-container = docker compose restart
cli-gateway-restart-hint-kubernetes = kubectl rollout restart deployment/voltd
cli-gateway-restart-hint-launchd = launchctl kickstart -k <your-voltd-label>
cli-gateway-restart-hint-process = перезапустите процесс `voltd daemon`
cli-gateway-restart-hint-systemd = systemctl restart voltd
cli-gateway-running-q = {"   "}Шлюз запущен? Запустите его командой:

# --- pairing ---
cli-pairing-check-enabled = Проверьте, включено ли сопряжение, затем запросите новый код устройства:
cli-pairing-disabled = ⚠️  Сопряжение шлюза отключено в конфигурации.
cli-pairing-enable-config = Чтобы включить сопряжение, задайте [gateway] require_pairing = true.
cli-pairing-enabled = 🔐 Сопряжение шлюза включено.
cli-pairing-fetch-failed = ❌ Не удалось получить код сопряжения от шлюза {$endpoint}
cli-pairing-inspect = Чтобы проверить работающий шлюз:
cli-pairing-new-code-unavailable = Шлюз не выдал новый код сопряжения. Возможно, код уже ожидает ввода, или сопряжение нужно сбросить.
cli-pairing-no-code = 🔐 Сопряжение шлюза включено, но активного кода сопряжения нет.
cli-pairing-pair-another = Чтобы сопрячь еще одно устройство, выполните:
cli-pairing-post = {"    "}POST /pair с заголовком X-Pairing-Code: {$code}
cli-pairing-requests-accepted = Все запросы будут приниматься без аутентификации.
cli-pairing-restart = {"   "}Перезапустите шлюз, чтобы создать новый код сопряжения.
cli-pairing-retry-or-rotate = Повторите попытку чуть позже или отзовите существующие сопряжения и выпустите новый код:
cli-pairing-revoke-replace = Чтобы отозвать существующие сопряжения и выпустить новый код, выполните:
cli-pairing-rotate-no-code = Запрос ротации завершился, но новый код не вернулся.
cli-pairing-show-only = `voltd gateway get-paircode` только показывает существующий активный код; новый он не выпускает.
cli-pairing-use-code = {"  "}Используйте этот одноразовый код, чтобы сопрячь новое устройство:

# --- config validation warnings surfaced to the panel ---
cli-config-review-hint = Выполните `voltd config list`, чтобы проверить настройки, затем задайте обязательные поля.
cli-config-section-degraded = предупреждение: раздел конфигурации `{$section}` в {$path} некорректен и на этот запуск сброшен к значениям по умолчанию. Значения в этом разделе НЕ действуют. Выполните `config migrate` исполняемым файлом `{$executable}`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-config-section-degraded-executable = предупреждение: раздел конфигурации `{$section}` в {$path} некорректен и на этот запуск сброшен к значениям по умолчанию. Значения в этом разделе НЕ действуют. Выполните `config migrate` исполняемым файлом `{$executable}`, чтобы увидеть ошибку разбора, затем исправьте файл.
cli-config-section-retired-node-transport = предупреждение: устаревший раздел конфигурации `[node_transport]` игнорируется, так как старый HMAC-транспорт узлов удален. Удалите раздел из config.toml.
cli-config-section-retired-wati = предупреждение: устаревший раздел конфигурации канала WATI `{$section}` игнорируется, так как поддержка WATI удалена. Перейдите на `[channels.whatsapp.<alias>]` через Cloud API или WhatsApp Web, затем отзовите неиспользуемый API-токен WATI.
channel-needs-quickstart-reply = Этот агент еще не настроен полностью. Оператору нужно выполнить Quickstart, прежде чем я смогу отвечать.

# --- RPC auth / config errors (panel-visible) ---
rpc-auth-alias-not-entitled = У субъекта нет прав на запрошенного агента
rpc-auth-assurance-required = Уровень подтверждения аутентификации недостаточен (требуется MFA/ACR)
rpc-auth-credential-expired = Учетные данные истекли: повторите initialize со свежим токеном
rpc-auth-credential-rejected = Учетные данные отклонены
rpc-auth-first-call-initialize = Первым вызовом должен быть 'initialize'
rpc-auth-local-roster-required = Настроен локальный список пользователей: подключитесь с сопоставленного uid или передайте auth_token в initialize
rpc-auth-misconfigured = Аутентификация на этом демоне настроена неверно (отказ по умолчанию)
rpc-auth-not-entitled = Аутентификация пройдена, но ни один профиль прав ничего не разрешает этой учетной записи
rpc-auth-pairing-revoked = Токен сопряжения отозван: выполните сопряжение и initialize заново
rpc-auth-remote-token-required = Удаленные подключения должны передавать auth_token в initialize
rpc-auth-required-token = Требуется аутентификация: передайте auth_token в initialize или подключитесь с сопоставленного локального uid
rpc-auth-revalidation-due = Требуется повторная проверка учетных данных: повторите initialize
rpc-auth-unknown-provider = Неизвестный выбор auth_provider
rpc-config-set-many-empty = config/set-many требует хотя бы одну запись в `sets`
rpc-config-set-many-entry-rejected = запись { $index } (`{ $prop }`) в config/set-many отклонена; ничего не сохранено: { $reason }
rpc-config-set-many-limit = config/set-many принимает не более { $limit } записей в `sets`; получено { $count }

# --- SOP approvals over RPC/WS ---
cli-sop-ws-engine-lock-poisoned = Блокировка движка SOP повреждена
cli-sop-ws-invalid-approval = sop approval_response требует run_id и решение approve или deny
cli-sop-ws-resolve-failed = sop resolve не удался: {$error}
cli-sop-ws-subsystem-disabled = Подсистема SOP не включена
sop-rpc-decision-invalid-state = Запуск {$run_id} нельзя разрешить в его текущем состоянии.
sop-rpc-decision-unauthorized = Субъект RPC не уполномочен разрешать этот шаг SOP.
sop-rpc-policy-missing = Политика одобрения SOP '{$name}' не настроена.
sop-rpc-policy-unavailable = Отложенная политика SOP недоступна: {$reason}.
cli-quickstart-terminal-size-unknown = Мастер не смог определить размер терминала и не может проверить, помещается ли список шагов. Запустите его из терминала, который сообщает свои размеры, или настройте без интерфейса командой `voltd config set <path> <value>`.
cli-quickstart-terminal-resized = Пока список шагов мастера был открыт, размер терминала изменился с {$initial_width}x{$initial_height} на {$current_width}x{$current_height}. Откройте список заново, чтобы продолжить.
cli-quickstart-empty-checklist = Мастер не может открыть пустой список шагов.
cli-quickstart-terminal-too-narrow = Мастеру нужен терминал шириной не менее {$min_width} колонок; сейчас {$width}. Расширьте терминал и повторите.
cli-quickstart-terminal-too-short = Мастеру нужен терминал высотой не менее {$min_height} строк; сейчас {$height}. Увеличьте высоту терминала и повторите.
