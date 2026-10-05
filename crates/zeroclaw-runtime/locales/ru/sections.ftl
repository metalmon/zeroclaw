# Русский перевод строк пикеров конфигурации и пресетов мастера настройки.
# Накладывается на английскую базу (sections.ftl) по ключам.

# Бэкенды памяти
picker-memory-sqlite = SQLite с векторным поиском (рекомендуется) - быстрый гибридный поиск, эмбеддинги
picker-memory-lucid = Мост Lucid Memory - синхронизация с локальным lucid-memory CLI, запасной SQLite
picker-memory-postgres = PostgreSQL - удаленное надежное хранилище через [storage.model_provider.config]
picker-memory-markdown = Markdown-файлы - просто, человекочитаемо, без зависимостей
picker-memory-none = Без памяти - отключить постоянную память

# Профили риска (быстрый старт)
picker-risk-locked_down = Строгий режим
picker-risk-locked_down-desc = Максимально строгие настройки. Доступ к файловой системе только в рабочей папке, подтверждение для среднего и высокого риска, без проброса переменных окружения в shell.
picker-risk-balanced = Сбалансированный
picker-risk-balanced-desc = Надежный вариант на каждый день для личной машины разработки. Под надзором, ограничен рабочей папкой, чувствительные пути заблокированы, песочница включена. Любая команда выполняется без белого списка, но команды высокого риска остаются заблокированы, пока не разрешены явно. Рекомендуется большинству.
picker-risk-yolo = Режим YOLO
picker-risk-yolo-desc = Полная автономия. Без подтверждений, без черного списка команд, без ограничения рабочей папкой. Выбирайте, только если понимаете, что делаете, на машине, которую не жалко сломать.

# Профили среды выполнения (быстрый старт)
picker-runtime-tight = Экономный
picker-runtime-tight-desc = Малые лимиты и короткие таймауты. Подходит для дешевых моделей, тарифицируемых API-ключей или коротких циклов обратной связи, когда агент должен рано остановиться и спросить, а не жечь бюджет.
picker-runtime-local_small = Локальный компактный
picker-runtime-local_small-desc = Компактный профиль без текстового запасного варианта для небольших локальных моделей. Держит контекст и результаты инструментов небольшими, отключает параллельный вызов инструментов и требует нативных или структурированных вызовов.
picker-runtime-balanced = Сбалансированный
picker-runtime-balanced-desc = Умеренные рабочие настройки по умолчанию. Подходят большинству в большинстве случаев.
picker-runtime-unbounded = Без ограничений
picker-runtime-unbounded-desc = Широкие лимиты и длинные таймауты. Выбирайте, когда активно ведете агента через сложную задачу и не хотите, чтобы он притормаживал.

# Бэкенды хранилища
picker-storage-sqlite-desc = Безопасный вариант по умолчанию для однонодовых установок: файловое хранилище, без настройки, без внешних сервисов.
picker-storage-postgres-desc = Для общих или многоэкземплярных развертываний, которым нужно надежное серверное хранилище.
picker-storage-qdrant-desc = Векторная база данных для семантического поиска, если Qdrant уже запущен.
picker-storage-markdown-desc = Человекочитаемые файлы с простым локальным хранением, без сервиса базы данных.
picker-storage-lucid-desc = Мост к локальному lucid-memory CLI с сохранением локальной работы в стиле SQLite.

# Провайдеры / туннель
picker-provider-local-desc = Локально - API-ключ не требуется
picker-tunnel-none-desc = Только localhost - без публичного туннеля.

# Integrations page: one description per catalog entry, keyed by display name slug.
integration-telegram-desc = подключите своего бота
integration-discord-desc = подключите своего бота
integration-slack-desc = подключите своего бота
integration-mattermost-desc = подключите своего бота
integration-imessage-desc = только macOS
integration-matrix-desc = собственный сервер чата
integration-signal-desc = открытый мессенджер с шифрованием
integration-whatsapp-desc = Business Cloud API
integration-whatsapp-web-desc = WhatsApp Web напрямую (wa-rs)
integration-linq-desc = iMessage/RCS/SMS через Linq API
integration-nextcloud-talk-desc = платформа NextCloud Talk
integration-email-desc = почта по IMAP/SMTP
integration-gmail-push-desc = push-уведомления Gmail через Pub/Sub
integration-twitch-desc = чат Twitch (IRC)
integration-irc-desc = IRC через TLS
integration-lark-desc = бот Lark
integration-dingtalk-desc = DingTalk в режиме Stream
integration-wecom-desc = вебхук бота WeCom
integration-wecom-websocket-desc = длинное соединение AI-бота WeCom
integration-wechat-desc = бот WeChat iLink
integration-qq-official-desc = бот Tencent QQ
integration-nostr-desc = личные сообщения Nostr
integration-clawdtalk-desc = канал ClawdTalk
integration-reddit-desc = бот Reddit (OAuth2)
integration-bluesky-desc = протокол AT
integration-git-desc = Git-хостинг (GitHub, Gitea, Forgejo): задачи, PR и события
integration-x-twitter-desc = бот X/Twitter через API v2
integration-mochat-desc = служба поддержки Mochat
integration-line-desc = подключите своего бота LINE
integration-voice-call-desc = исходящие голосовые звонки
integration-voicewake-desc = активация голосом по ключевому слову
integration-mqtt-desc = слушатель СОП по MQTT
integration-amqp-desc = подписчик тем AMQP
integration-filesystem-desc = слушатель СОП по изменениям файлов
integration-webhook-desc = HTTP-эндпоинт
integration-plugin-desc = установленный WASM-плагин канала
integration-browser-desc = открывает ссылки и управляет Chrome/Chromium
integration-google-workspace-desc = Drive, Gmail, Calendar, Sheets, Docs через gws CLI
integration-shell-desc = выполнение команд в терминале
integration-file-system-desc = чтение и запись файлов
integration-weather-desc = прогноз и текущая погода (wttr.in)
integration-spawn-subagent-desc = запускает временного субагента с личностью этого агента
