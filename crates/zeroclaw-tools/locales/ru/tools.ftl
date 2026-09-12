tool-backup = Создание, просмотр, проверка и восстановление резервных копий рабочей области

tool-browser = Автоматизация веб-браузера с подключаемыми бэкендами (agent-browser, rust-native, computer_use). Поддерживает действия с DOM, а также опциональные действия на уровне ОС (mouse_move, mouse_click, mouse_drag, key_type, key_press, screen_capture) через сайдкар computer-use. Используйте 'snapshot', чтобы сопоставить интерактивные элементы со ссылками (@e1, @e2). Применяет ограничение browser.allowed_domains для действий открытия страниц.

tool-browser-delegate = Делегирует задачи в браузере CLI-инструменту с поддержкой браузера для работы с веб-приложениями, такими как Teams, Outlook, Jira, Confluence

tool-browser-open = Открывает разрешённый HTTPS-адрес в системном браузере. Ограничения безопасности: только домены из белого списка, без локальных/приватных хостов, без скрапинга.

tool-channel-room = Создаёт комнаты и приглашает пользователей через активный канал. Укажите ключ канала, например 'matrix.default', действие 'create_room' или 'invite_user', и поля, специфичные для этого действия.
tool-channel-room-param-action = Действие по управлению комнатой, которое нужно выполнить.
tool-channel-room-param-channel = Ключ активного канала, например 'matrix.default'.
tool-channel-room-param-name = Необязательное название комнаты для create_room.
tool-channel-room-param-topic = Необязательная тема комнаты для create_room.
tool-channel-room-param-invites = Необязательные ID пользователей для приглашения при создании комнаты.
tool-channel-room-param-visibility = Необязательная видимость комнаты для create_room.
tool-channel-room-param-encryption = Запрашивать ли шифрование комнаты при create_room.
tool-channel-room-param-room-id = ID существующей комнаты для invite_user.
tool-channel-room-param-user-id = ID пользователя для приглашения в invite_user.
tool-channel-room-error-security = Действие заблокировано: { $err }
tool-channel-room-error-invalid-action = Недопустимое действие '{ $action }': должно быть 'create_room' или 'invite_user'.
tool-channel-room-error-not-initialized = Каналы пока недоступны (каналы не инициализированы).
tool-channel-room-error-channel-not-found = Канал '{ $channel }' не найден. Доступные каналы: { $available }
tool-channel-room-error-create-failed = Не удалось создать комнату: { $err }
tool-channel-room-error-invite-failed = Не удалось пригласить пользователя: { $err }
tool-channel-room-error-invites-array = 'invites' должен быть массивом строк.
tool-channel-room-error-invites-item = 'invites' должен быть массивом непустых строк.
tool-channel-room-error-invalid-visibility = Недопустимая видимость комнаты: { $err }
tool-channel-room-error-missing-param = Отсутствует параметр '{ $param }'.
tool-channel-room-error-string-param = '{ $param }' должен быть строкой.
tool-channel-room-error-bool-param = '{ $param }' должен быть булевым значением.

tool-cloud-ops = Консультационный инструмент по облачной трансформации. Анализирует планы IaC, оценивает пути миграции, проверяет затраты и сверяет архитектуру с принципами Well-Architected Framework. Только чтение: не создаёт и не изменяет облачные ресурсы.

tool-cloud-patterns = Библиотека облачных паттернов. По описанию нагрузки предлагает применимые cloud-native архитектурные паттерны (контейнеризация, serverless, модернизация баз данных и т.д.).

tool-composio = Выполняет действия в 1000+ приложениях через Composio (Gmail, Notion, GitHub, Slack и др.). Используйте action='list', чтобы увидеть доступные действия (включая имена параметров). action='execute' с action_name/tool_slug и params запускает действие. Если точные параметры неизвестны, передайте вместо них 'text' с описанием желаемого на естественном языке (Composio определит нужные параметры через NLP). action='list_accounts' или action='connected_accounts' выводит список аккаунтов, подключённых через OAuth. action='connect' с app/auth_config_id возвращает URL для OAuth. connected_account_id определяется автоматически, если не указан.

tool-content-search = Поиск по содержимому файлов регулярным выражением в пределах рабочей области. Использует ripgrep (rg), grep или внутренний резервный вариант. Режимы вывода: 'content' (совпадающие строки с контекстом), 'files_with_matches' (только пути к файлам), 'count' (число совпадений по каждому файлу). Пример: pattern='fn main', include='*.rs', output_mode='content'.

tool-cron-add = Создаёт запланированное cron-задание (shell или агент) с расписанием cron/at/every. Используйте job_type='agent' с prompt, чтобы запускать ИИ-агента по расписанию. Чтобы доставить результат в канал (Discord, Telegram, Slack, Mattermost, Matrix), укажите delivery={"{"}"mode":"announce","channel":"discord","to":"<channel_id_or_chat_id>"{"}"}. Это предпочтительный инструмент для отправки отложенных/запланированных сообщений пользователям через каналы.

tool-cron-list = Выводит список всех запланированных cron-заданий

tool-cron-remove = Удаляет cron-задание по id

tool-cron-run = Немедленно принудительно запускает cron-задание и записывает историю запуска

tool-cron-runs = Выводит недавнюю историю запусков cron-задания

tool-cron-update = Изменяет существующее cron-задание (расписание, команду, prompt, включено/выключено, delivery, модель и т.д.)

tool-data-management = Хранение данных рабочей области, их очистка и статистика по хранилищу

tool-delegate = Делегирует подзадачу специализированному агенту. Используйте, если задаче нужна другая модель (например, быстрое резюмирование, глубокое рассуждение, генерация кода). По умолчанию суб-агент выполняет один prompt; при agentic=true он может выполнять итерации с отфильтрованным циклом вызовов инструментов.

tool-file-edit = Редактирует файл, заменяя точное совпадение строки на новое содержимое

tool-file-download = Скачивает файл с настроенного удалённого эндпоинта и записывает его в рабочую область агента. Укажите идентификатор документа для загрузки и путь назначения относительно рабочей области; URL эндпоинта задаётся конфигурацией хоста и никогда не управляется моделью. Байты передаются напрямую на диск и не загружаются в контекст модели. Возвращает HTTP-статус, число записанных байт и путь назначения.
tool-file-download-param-document-id = Идентификатор документа для загрузки с настроенного эндпоинта.
tool-file-download-param-dest-path = Путь относительно рабочей области, куда будет записан файл. Родительский каталог должен уже существовать.
tool-file-download-error-disabled = file_download отключён: [file_download].url не настроен
tool-file-download-error-read-only = Действие заблокировано: режим автономности — только чтение
tool-file-download-error-rate-limited-hour = Превышен лимит запросов: слишком много действий за последний час
tool-file-download-error-rate-limited-budget = Превышен лимит запросов: исчерпан бюджет действий
tool-file-download-error-missing-document-id = Отсутствует параметр 'document_id'
tool-file-download-error-missing-dest-path = Отсутствует параметр 'dest_path'
tool-file-download-error-invalid-file-name = Недопустимый dest_path '{ $dest_path }': должен заканчиваться конкретным именем файла
tool-file-download-error-no-parent = Недопустимый dest_path '{ $dest_path }': нет родительского каталога
tool-file-download-error-resolve-dir = Не удалось определить каталог назначения для '{ $dest_path }': { $err }
tool-file-download-error-client-build = Не удалось создать клиент загрузки: { $err }
tool-file-download-error-request = Запрос на загрузку завершился ошибкой: { $err }
tool-file-download-error-status = Эндпоинт загрузки вернул статус { $status }
tool-file-download-error-too-large-reported = Загрузка слишком велика: эндпоинт сообщает { $len } байт (лимит: { $limit } байт)
tool-file-download-error-too-large-stream = Загрузка слишком велика: превышен лимит { $limit } байт
tool-file-download-error-temp-create = Не удалось создать временный файл загрузки: { $err }
tool-file-download-error-read-body = Ошибка при чтении тела ответа: { $err }
tool-file-download-error-write-body = Ошибка при записи загруженных байт: { $err }
tool-file-download-error-flush = Не удалось сбросить буфер загруженного файла: { $err }
tool-file-download-error-move = Не удалось переместить загруженный файл на место: { $err }
tool-file-download-success = Загружено { $written } байт в { $dest_path } ({ $status })

tool-file-read = Читает содержимое файла с номерами строк. Поддерживает частичное чтение через offset и limit. Бинарные и графические файлы отклоняются (для изображений используйте инструмент image_info). Установите encoding="base64", чтобы вернуть исходные байты в кодировке base64 (для бинарных файлов вроде .pdf/.xlsx/.docx); в этом режиме offset/limit игнорируются.

tool-file-write = Записывает содержимое в файл рабочей области

tool-git-operations = Выполняет структурированные операции Git (status, diff, log, branch, commit, add, checkout, stash). Возвращает разобранный JSON-вывод и учитывает политику безопасности для контроля автономности.
tool-git-operations-error-not-in-repo = Путь '{ $path }' не является Git-репозиторием. Укажите путь внутри рабочего дерева Git, передайте 'path' для подкаталога репозитория или инициализируйте репозиторий перед запуском git_operations.

tool-git-forge-error-requires-field = { $resource }.{ $action } требует '{ $field }'.
tool-git-forge-error-requires-number = { $resource }.{ $action } требует 'number'.
tool-git-forge-error-issue-close-reason = issue.close: 'reason' должен быть 'completed' или 'not_planned'.
tool-git-forge-error-pull-merge-method = pull.merge: 'method' должен быть 'merge', 'squash' или 'rebase'.
tool-git-forge-error-review-verdict = review.create: 'verdict' должен быть approve|request_changes|comment, получено '{ $verdict }'.
tool-git-forge-error-unknown-cell = неизвестный или неподдерживаемый resource/action '{ $resource }.{ $action }'. Вызовите action 'describe', чтобы увидеть поддерживаемую таблицу, либо используйте 'raw' для всего, что не перечислено.
tool-git-forge-error-no-channels = Каналы пока недоступны (каналы не инициализированы).
tool-git-forge-error-raw-requires-method = 'raw' требует 'method'.
tool-git-forge-error-raw-requires-path = 'raw' требует 'path'.
tool-git-forge-error-requires-resource = типизированный вызов требует 'resource' (либо используйте action 'raw'/'describe').
tool-git-forge-error-missing-repo = Отсутствует 'repo' (ожидается 'owner/repo').

tool-glob-search = Ищет файлы, соответствующие glob-шаблону, в пределах рабочей области. Возвращает отсортированный список путей к найденным файлам относительно корня рабочей области. Примеры: '**/*.rs' (все файлы Rust), 'src/**/mod.rs' (все mod.rs в src).

tool-google-workspace = Работа с сервисами Google Workspace (Drive, Gmail, Calendar, Sheets, Docs и др.) через CLI gws. Требует установленного и аутентифицированного gws.

tool-hardware-board-info = Возвращает полную информацию о плате (чип, архитектура, карта памяти) для подключённого оборудования. Используйте, когда пользователь спрашивает 'board info', 'what board do I have', 'connected hardware', 'chip info', 'what hardware' или 'memory map'.

tool-hardware-memory-map = Возвращает карту памяти (диапазоны адресов flash и RAM) для подключённого оборудования. Используйте, когда пользователь спрашивает 'upper and lower memory addresses', 'memory map', 'address space' или 'readable addresses'. Возвращает диапазоны flash/RAM из документации на чип.

tool-hardware-memory-read = Читает реальные значения памяти/регистров с платы Nucleo через USB. Используйте, когда пользователь просит 'read register values', 'read memory at address', 'dump memory', 'lower memory 0-126' или 'give address and value'. Возвращает hex-дамп. Требует подключённую по USB плату Nucleo и функцию probe. Параметры: address (в hex, например 0x20000000 для начала RAM), length (в байтах, по умолчанию 128).

tool-http-request = Выполняет HTTP-запросы к внешним API. Поддерживает методы GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS. Ограничения безопасности: только домены из белого списка, без локальных/приватных хостов, настраиваемые таймаут и лимит размера ответа.

tool-image-info = Читает метаданные файла изображения (формат, размеры, размер файла) и опционально возвращает данные в кодировке base64.

tool-jira = Работа с Jira: чтение задач, поиск по JQL, добавление комментариев, просмотр списка проектов и доступных переходов задачи, перевод задачи по workflow и создание новых задач.

tool-knowledge = Управляет графом знаний: архитектурные решения, паттерны решений, извлечённые уроки, эксперты и связи между ними.

tool-linkedin = Работа с LinkedIn: создание постов, просмотр списка своих постов, комментирование, реакции, удаление постов, просмотр вовлечённости, получение информации о профиле и чтение настроенной контент-стратегии. Требует учётных данных LINKEDIN_* в файле .env.

tool-discord-search = Поиск по истории сообщений Discord, сохранённой в discord.db. Используйте, чтобы найти прошлые сообщения, обобщить активность канала или посмотреть, что говорили пользователи. Поддерживает поиск по ключевым словам и необязательные фильтры: channel_id, since, until.

tool-memory-forget = Удаляет запись из памяти по ключу. Используйте для удаления устаревших фактов или чувствительных данных. Возвращает, была ли запись найдена и удалена.

tool-memory-recall = Поиск в долговременной памяти релевантных фактов, предпочтений или контекста. Возвращает результаты, ранжированные по релевантности. Не указывайте query или передайте просто *, чтобы получить последние записи памяти.

tool-memory-store = Сохраняет факт, предпочтение или заметку в долговременной памяти. Используйте категорию 'core' для постоянных фактов, 'daily' для заметок сессии, 'conversation' для контекста переписки, либо произвольное имя категории.

tool-microsoft365 = Интеграция с Microsoft 365: управление почтой Outlook, сообщениями Teams, событиями Calendar, файлами OneDrive и поиском по SharePoint через Microsoft Graph API

tool-model-routing-config = Управляет настройками модели по умолчанию, сценарными маршрутами provider/model, правилами классификации и именованными профилями агентов

tool-notion = Работа с Notion: запросы к базам данных, чтение/создание/обновление страниц и поиск по рабочему пространству.


tool-project-intel = Аналитика по ходу проекта: формирование статус-отчётов, выявление рисков, подготовка черновиков обновлений для клиента, резюмирование спринтов и оценка трудозатрат. Инструмент только для анализа, без изменений.

tool-proxy-config = Управляет настройками прокси ZeroClaw (область: environment | zeroclaw | services), включая применение переменных окружения к среде выполнения и процессам

tool-pushover = Отправляет уведомление Pushover на ваше устройство. Требует PUSHOVER_TOKEN и PUSHOVER_USER_KEY в файле .env.

tool-schedule = Управляет запланированными задачами только для shell. Действия: create/add/once/list/get/cancel/remove/pause/resume. ВНИМАНИЕ: этот инструмент создаёт shell-задания, вывод которых только логируется и НЕ доставляется в какой-либо канал. Чтобы отправить запланированное сообщение в Discord/Telegram/Slack/Matrix, используйте инструмент cron_add с job_type='agent' и конфигурацией delivery вида {"{"}"mode":"announce","channel":"discord","to":"<channel_id>"{"}"}.

tool-screenshot = Делает снимок текущего экрана. Возвращает путь к файлу и данные PNG в кодировке base64.
tool-browser-screenshot-error-path-not-allowed = Путь снимка экрана '{ $path }' отсутствует в белом списке рабочей области
tool-browser-screenshot-error-parent-not-exist = Родительский каталог '{ $parent }' пути снимка экрана '{ $path }' не существует
tool-browser-screenshot-error-path-outside-workspace = Путь снимка экрана '{ $path }' указывает на '{ $canonical }', что находится вне рабочей области
tool-browser-screenshot-error-missing-filename = В пути снимка экрана '{ $path }' отсутствует имя файла
tool-browser-screenshot-error-runtime-config-target = Невозможно записать снимок экрана по пути конфигурации среды выполнения '{ $target }'
tool-browser-screenshot-error-symlink-target = Невозможно записать снимок экрана в цель символической ссылки '{ $target }'
tool-browser-screenshot-error-path-not-utf8 = Путь снимка экрана '{ $path }' указывает на не-UTF-8 путь; запись через некорректное преобразование отклонена
tool-browser-screenshot-error-computeruse-non-string-path = Параметр 'path' снимка экрана должен быть строкой, получено { $path }
tool-browser-screenshot-error-non-string-path = 'path' снимка экрана должен быть строкой либо отсутствовать
tool-browser-screenshot-error-args-not-object = Аргументы снимка экрана должны быть JSON-объектом
tool-browser-screenshot-error-sidecar-no-png-data = Сайдкар computer-use не вернул данные PNG
tool-browser-screenshot-error-sidecar-empty-png = Сайдкар computer-use вернул пустой снимок экрана
tool-browser-screenshot-error-sidecar-not-png = Сайдкар computer-use вернул снимок экрана не в формате PNG
tool-browser-screenshot-error-sidecar-non-json-success = Сайдкар computer-use вернул успешный ответ не в формате JSON для снимка экрана с указанным путём; запрошенный файл не был записан

tool-security-ops = Инструмент для операций безопасности в управляемых сервисах кибербезопасности. Действия: triage_alert (классификация/приоритизация оповещений), run_playbook (выполнение шагов реагирования на инцидент), parse_vulnerability (разбор результатов сканирования), generate_report (формирование отчётов о состоянии безопасности), list_playbooks (список доступных плейбуков), alert_stats (сводная статистика по оповещениям).

tool-shell = Выполняет команду shell в каталоге рабочей области

tool-sop-advance = Сообщает результат текущего шага СОП и переходит к следующему шагу. Укажите run_id, успешность выполнения шага и краткое резюме результата.

tool-sop-approve = Утверждает шаг СОП, ожидающий подтверждения оператора. Возвращает инструкцию для выполнения шага. Используйте sop_status, чтобы увидеть, какие запуски ожидают подтверждения.

tool-sop-execute = Запускает вручную стандартную операционную процедуру (СОП) по имени. Возвращает ID запуска и инструкцию первого шага. Используйте sop_list, чтобы увидеть доступные СОП.

tool-sop-list = Выводит список всех загруженных стандартных операционных процедур (СОП) с их триггерами, приоритетом, числом шагов и числом активных запусков. Можно отфильтровать по имени или приоритету.

tool-sop-status = Запрашивает статус выполнения СОП. Укажите run_id для конкретного запуска или sop_name, чтобы вывести запуски по этой СОП. Без аргументов показывает все активные запуски.

tool-tool-search = Получает полные схемы отложенных MCP-инструментов, чтобы их можно было вызвать. Используйте "select:name1,name2" для точного совпадения или ключевые слова для поиска.

tool-web-fetch = Загружает веб-страницу и возвращает её содержимое в виде чистого текста. HTML-страницы автоматически преобразуются в читаемый текст. Ответы в формате JSON и обычный текст возвращаются без изменений. Только GET-запросы; следует редиректам. Безопасность: только домены из белого списка, без локальных/приватных хостов.

tool-web-search-tool = Ищет информацию в интернете. Возвращает релевантные результаты поиска с заголовками, URL и описаниями. Используйте, чтобы найти актуальную информацию, новости или материалы по теме.
tool-web-search-tool-error-duckduckgo-blocked = DuckDuckGo ограничивает частоту запросов с этой машины. Не повторяйте и не переформулируйте запрос; подождите несколько минут, загрузите известные URL напрямую через web_fetch, либо настройте SearXNG, Brave или Tavily в качестве провайдера web_search.
tool-web-search-tool-error-searxng-not-configured = URL инстанса SearXNG не настроен. Укажите [web_search] searxng_instance_url в config.toml либо переопределите переменной окружения ZEROCLAW_web_search__searxng_instance_url.
tool-web-search-tool-note-truncated-results = (остальные результаты опущены)

tool-workspace = Управляет рабочими областями для нескольких клиентов. Подкоманды: list, switch, create, info, export. Каждая рабочая область имеет изолированные память, аудит, секреты и ограничения инструментов.

tool-weather = Возвращает текущую погоду и прогноз для любой точки мира. Поддерживает названия городов (на любом языке и в любой письменности), коды аэропортов IATA (например, 'LAX'), GPS-координаты (например, '51.5,-0.1'), почтовые индексы и геолокацию по домену. Возвращает температуру, ощущаемую температуру, влажность, скорость/направление ветра, осадки, видимость, давление, УФ-индекс и облачность. Доступен прогноз на 0-3 дня с почасовой разбивкой. По умолчанию используются метрические единицы (°C, км/ч, мм), но можно задать имперские (°F, миль/ч, дюймы) для каждого запроса. API-ключ не требуется.
