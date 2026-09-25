// GENERATED (machine translation) by _local/i18n-mt/translate-tools + apply-tools from live /api/tools.
// RU tool-arg help; EN fallback via the localize* helpers. Tool top-level descriptions are already RU from the daemon.
import { getLocale } from './i18n';

/** tool name -> arg name -> RU description. */
export const toolArgDescriptionsRu: Record<string, Record<string, string>> = {
  "shell": {
    "approved": "Установите true, чтобы явно одобрить команды со средним/высоким риском в режиме supervised",
    "command": "Команда shell для выполнения"
  },
  "file_read": {
    "encoding": "Кодировка вывода (по умолчанию: 'utf8'). Используйте 'base64' для чтения бинарных файлов в виде байтов, закодированных в base64.",
    "limit": "Максимальное количество строк для возврата (по умолчанию: все). Игнорируется, если encoding равен 'base64'.",
    "offset": "Номер начальной строки (нумерация с 1, по умолчанию: 1). Игнорируется, если encoding равен 'base64'.",
    "path": "Путь к файлу. Относительные пути разрешаются от корня workspace; абсолютные пути должны находиться внутри workspace."
  },
  "file_write": {
    "content": "Содержимое для записи. Текст в UTF-8, если encoding равен 'utf8'; байты, закодированные в base64, если encoding равен 'base64'.",
    "encoding": "Как интерпретировать 'content' перед записью (по умолчанию: 'utf8'). Используйте 'base64' для бинарных файлов.",
    "path": "Путь к файлу. Относительные пути разрешаются от workspace; пути за его пределами требуют наличия в policy allowlist."
  },
  "file_edit": {
    "path": "Путь к файлу. Относительные пути разрешаются от workspace; пути за его пределами требуют наличия в policy allowlist.",
    "new_string": "Текст замены (пустая строка для удаления найденного текста)",
    "old_string": "Точный текст для поиска и замены (должен встречаться в файле ровно один раз)"
  },
  "glob_search": {
    "pattern": "Glob-шаблон для поиска файлов, например '**/*.rs', 'src/**/mod.rs'"
  },
  "content_search": {
    "case_sensitive": "Учет регистра при сопоставлении. По умолчанию true",
    "context_after": "Количество строк контекста после каждого совпадения (только в режиме content)",
    "context_before": "Количество строк контекста перед каждым совпадением (только в режиме content)",
    "include": "Glob-фильтр файлов, например '*.rs', '*.{ts,tsx}'",
    "max_results": "Максимальное количество результатов для возврата. По умолчанию 1000",
    "multiline": "Включить многострочное сопоставление (только для ripgrep, вызывает ошибку при резервном варианте без ripgrep)",
    "output_mode": "Формат вывода: 'content' (совпадающие строки), 'files_with_matches' (только пути), 'count' (количество совпадений)",
    "path": "Каталог для поиска, относительно корня workspace. По умолчанию '.'",
    "pattern": "Регулярное выражение для поиска"
  },
  "cron_add": {
    "allowed_tools": "Необязательный allowlist имен инструментов для agent-заданий. Если не указан, запуски агента через cron сохраняют доступ к инструментам, не относящимся к планировщику, но исключают инструменты изменения планировщика, такие как cron_add, cron_update, cron_remove, cron_run и schedule. Чтобы вернуть их, укажите эти имена явно.",
    "approved": "Установите true, чтобы явно одобрить команды shell со средним/высоким риском в режиме supervised",
    "command": "Команда shell для выполнения (обязательна, если job_type равен 'shell')",
    "delete_after_run": "Если true, задание автоматически удаляется после первого успешного выполнения. По умолчанию true для одноразовых расписаний 'at' и 'after'.",
    "delivery": "Необязательная конфигурация доставки для отправки вывода задания в канал после каждого запуска. При создании из хода чата опущенные поля по умолчанию направляют ответ обратно в этот диалог.",
    "job_type": "Тип задания: 'shell' выполняет команду, 'agent' запускает AI-агента с промптом",
    "model": "Необязательное переопределение модели для agent-заданий, например 'x-ai/grok-4-1-fast'",
    "name": "Необязательное человекочитаемое имя задания",
    "prompt": "Промпт агента для запуска по расписанию (обязателен, если job_type равен 'agent')",
    "schedule": "Когда запускать задание. Должна использоваться ровно одна из четырех форм. Для относительных одноразовых напоминаний предпочтительно 'after'.",
    "session_target": "Контекст сессии агента: 'isolated' запускает новую сессию при каждом выполнении, 'main' повторно использует основную сессию",
    "uses_memory": "Если true (по умолчанию), перед запуском agent-задания извлекается и внедряется контекст памяти. Установите false для stateless заданий digest/report, которые не должны накапливать или потреблять записи памяти."
  },
  "cron_update": {
    "approved": "Установите true, чтобы явно одобрить команды shell со средним/высоким риском в режиме supervised",
    "job_id": "ID или имя cron-задания для обновления. Принимает либо UUID, возвращенный cron_add/cron_list, либо человекочитаемое имя задания (без учета регистра). Предварительно вызывать cron_list не требуется.",
    "patch": "Поля для обновления. Указывайте только те поля, которые нужно изменить; опущенные поля остаются без изменений."
  },
  "cron_run": {
    "approved": "Установите true, чтобы явно одобрить команды shell со средним/высоким риском в режиме supervised"
  },
  "schedule": {
    "approved": "Установите true, чтобы явно одобрить команды shell со средним/высоким риском в режиме supervised",
    "action": "Действие для выполнения",
    "command": "Команда shell для выполнения. Обязательна для create/add/once.",
    "delay": "Задержка для одноразовых задач (например, '30m', '2h', '1d').",
    "expression": "Cron-выражение для повторяющихся задач (например, '*/5 * * * *').",
    "id": "Task ID. Обязателен для get/cancel/remove/pause/resume.",
    "run_at": "Абсолютное время в формате RFC3339 для одноразовых задач (например, '2030-01-01T00:00:00Z')."
  },
  "cron_remove": {
    "job_id": "ID или имя cron-задания для удаления. Принимает либо UUID, возвращенный cron_add/cron_list, либо человекочитаемое имя задания (без учета регистра). Предварительно вызывать cron_list не требуется."
  },
  "memory_store": {
    "category": "Категория памяти: 'core' (постоянная), 'daily' (сессия), 'conversation' (чат) или произвольное имя категории. По умолчанию 'core'.",
    "content": "Информация для запоминания",
    "key": "Уникальный ключ для этой записи памяти (например, 'user_lang', 'project_stack')"
  },
  "memory_recall": {
    "limit": "Максимальное количество результатов для возврата (по умолчанию: 5)",
    "query": "Ключевые слова или фраза для поиска в памяти. Опустите или передайте просто '*', чтобы получить последние записи памяти; составные термины с wildcard остаются поиском по ключевым словам.",
    "search_mode": "Стратегия поиска: bm25 (по ключевым словам), embedding (семантический) или hybrid (оба варианта). По умолчанию берется значение из конфигурации.",
    "since": "Фильтровать записи памяти, созданные в это время или позже (RFC 3339, например 2025-03-01T00:00:00Z)",
    "until": "Фильтровать записи памяти, созданные в это время или раньше (RFC 3339)"
  },
  "memory_forget": {
    "key": "Ключ записи памяти для удаления"
  },
  "memory_export": {
    "category": "Фильтр по категории: core, daily, conversation или произвольное имя.",
    "namespace": "Фильтр по namespace (граница изоляции agent/context).",
    "session_id": "Фильтр по session ID.",
    "since": "Нижняя граница (включительно) по created_at в формате RFC 3339. Пример: 2025-01-01T00:00:00Z",
    "until": "Верхняя граница (включительно) по created_at в формате RFC 3339. Пример: 2025-12-31T23:59:59Z"
  },
  "memory_purge": {
    "namespace": "Namespace для очистки. Удаляет все записи памяти, у которых поле namespace равно этому значению.",
    "session_id": "Session ID для очистки. Удаляет все записи памяти в этой сессии."
  },
  "spawn_subagent": {
    "prompt": "Задача или вопрос для SubAgent. Формулируйте конкретно и самодостаточно — SubAgent не видит историю этого диалога."
  },
  "send_message_to_peer": {
    "channel": "Ссылка на канал для доставки (например, 'telegram.prod'). Должен быть одним из настроенных каналов агента и каналом, который также прослушивает целевой peer.",
    "message": "Тело сообщения для доставки.",
    "target": "Идентификатор получателя — alias peer-агента или username внешнего peer (например, '@operator')."
  },
  "model_routing_config": {
    "agentic": "Включить режим цикла вызовов инструментов (tool-call loop) для aliased-агента",
    "allowed_tools": "Разрешенные инструменты для agentic-режима делегирования (строка или массив строк)",
    "api_key": "Необязательное переопределение API-ключа для маршрута сценария или aliased-агента",
    "classification_enabled": "Если true, выполняется upsert правила классификации для этой подсказки (hint); false удаляет его",
    "delegate_same_risk_profile": "Автоматически разрешать делегирование peer'ам с тем же профилем риска (по умолчанию true). Установите false, чтобы ограничить доступ явным списком делегатов.",
    "delegates": "Явный список делегатов. Принимает строку со значениями через запятую, массив строк или объекты вида {agent, mode}; mode может быть bounded или independent.",
    "hint": "Имя подсказки сценария (например: conversation, coding, reasoning)",
    "keywords": "Ключевые слова классификации для upsert_scenario (строка или массив строк)",
    "max_depth": "Максимальная глубина рекурсии делегирования",
    "max_iterations": "Максимальное количество итераций вызова инструментов для agentic-режима делегирования",
    "max_length": "Необязательное условие сопоставления по максимальной длине сообщения",
    "min_length": "Необязательное условие сопоставления по минимальной длине сообщения",
    "model": "Модель для set_default/upsert_scenario/upsert_agent",
    "model_provider": "ModelProvider для set_default/upsert_scenario/upsert_agent",
    "name": "Имя aliased-агента для upsert_agent/remove_agent",
    "patterns": "Литеральные шаблоны классификации для upsert_scenario (строка или массив строк)",
    "priority": "Приоритет классификации (чем выше значение, тем раньше выполняется)",
    "remove_classification": "При remove_scenario — удалять ли соответствующее правило классификации (по умолчанию true)",
    "temperature": "Необязательное переопределение temperature (0.0-2.0)"
  },
  "model_switch": {
    "action": "Действие для выполнения: получить состояние ожидающего переключения, установить переключение provider-profile/model во время выполнения, вывести список доступных семейств провайдеров или вывести список распространенных моделей для profile провайдера",
    "model": "Model ID (например, 'gpt-4o', 'claude-sonnet-4-6'). Обязателен для действия 'set'.",
    "model_provider": "Ссылка на profile провайдера через точку (например, 'openai.default', 'anthropic.sonnet', 'ollama.local'). Обязательна для действий 'set' и 'list_models'."
  },
  "proxy_config": {
    "all_proxy": "Резервный (fallback) URL прокси для всех протоколов",
    "clear_env": "При action=disable очищает переменные окружения прокси процесса",
    "enabled": "Включить или отключить прокси",
    "http_proxy": "URL прокси HTTP",
    "https_proxy": "URL прокси HTTPS",
    "no_proxy": "Строка со значениями через запятую или массив записей NO_PROXY",
    "scope": "Область действия прокси: environment | zeroclaw | services",
    "services": "Строка со значениями через запятую или массив селекторов служб, используемых при scope=services"
  },
  "git_operations": {
    "action": "Действие stash (для операции 'stash')",
    "branch": "Имя ветки (для операции 'checkout' или подкоманды 'worktree add')",
    "cached": "Показать staged-изменения (для операции 'diff')",
    "files": "Файл или путь для diff (для операции 'diff', по умолчанию: '.')",
    "include_untracked": "Для 'stash push': также добавлять в stash неотслеживаемые файлы (-u). Без этого `git stash push` затрагивает только отслеживаемые файлы.",
    "index": "Индекс stash (для 'stash' с действием 'drop')",
    "keep_index": "Для 'stash push': сохранить staged-изменения в рабочем дереве после stash — в stash попадают только unstaged-изменения.",
    "limit": "Количество записей лога (для операции 'log', по умолчанию: 10)",
    "message": "Сообщение коммита (для операции 'commit'); сообщение stash (для 'stash push', по умолчанию 'auto-stash')",
    "operation": "Git-операция для выполнения",
    "path": "Необязательный путь к подкаталогу внутри workspace, в котором выполняются git-операции. По умолчанию корень workspace.",
    "paths": "Пути к файлам через пробел. Для 'add' — файлы для staging. Для 'stash push' — pathspec'ы, ограничивающие область stash — без этого в stash попадает все рабочее дерево.",
    "subcommand": "Подкоманда worktree",
    "worktree_path": "Путь в файловой системе для worktree (для подкоманд 'worktree add' и 'worktree remove'). Относительные пути разрешаются внутри workspace; абсолютные пути должны оставаться внутри workspace или настроенных разрешенных корневых каталогов."
  },
  "pushover": {
    "message": "Сообщение уведомления для отправки",
    "priority": "Приоритет сообщения: -2 (самый низкий/беззвучный), -1 (низкий/без звука), 0 (обычный), 1 (высокий), 2 (экстренный/повторяющийся)",
    "sound": "Переопределение звука уведомления (например, 'pushover', 'bike', 'bugle', 'cashregister' и т. д.)",
    "title": "Необязательный заголовок уведомления"
  },
  "calculator": {
    "a": "Первый операнд. Обязателен для: pow, modulo, percentage_change.",
    "b": "Второй операнд. Обязателен для: pow, modulo, percentage_change.",
    "base": "Основание логарифма (по умолчанию: 10). Необязателен для: log.",
    "decimals": "Количество знаков после запятой для округления. Обязателен для: round.",
    "function": "Вычисление для выполнения. Арифметика: add(values), subtract(values), divide(values), multiply(values), pow(a,b), sqrt(x), abs(x), modulo(a,b), round(x,decimals). Логарифмические/экспоненциальные: log(x,base?), ln(x), exp(x), factorial(x). Агрегация: sum(values), average(values), count(values), min(values), max(values), range(values). Статистика: median(values), mode(values), variance(values), stdev(values), percentile(values,p). Утилитарные: percentage_change(a,b), clamp(x,min_val,max_val).",
    "max_val": "Верхняя граница. Обязателен для: clamp.",
    "min_val": "Нижняя граница. Обязателен для: clamp.",
    "p": "Ранг перцентиля (0-100). Обязателен для: percentile.",
    "values": "Массив числовых значений. Обязателен для: add, subtract, divide, multiply, sum, average, median, mode, min, max, range, variance, stdev, percentile, count.",
    "x": "Входное число. Обязателен для: sqrt, abs, exp, ln, log, factorial."
  },
  "weather": {
    "days": "Количество дней прогноза (0–3). 0 возвращает только текущие условия. По умолчанию: 1.",
    "location": "Местоположение для получения погоды. Принимает названия городов на любом языке/в любой письменности, коды аэропортов IATA, GPS-координаты (например, '35.6762,139.6503'), почтовые/zip-коды или доменное имя для геолокации (например, 'stackoverflow.com').",
    "units": "Система единиц измерения. 'metric' = °C, км/ч, мм (по умолчанию). 'imperial' = °F, миль/ч, дюймы."
  },
  "canvas": {
    "action": "Действие для выполнения на canvas.",
    "canvas_id": "Уникальный идентификатор canvas. По умолчанию 'default'.",
    "content": "Содержимое для рендеринга (для действия render).",
    "content_type": "Тип содержимого для действия render: html, svg, markdown или text.",
    "expression": "Выражение JavaScript для вычисления (для действия eval). Результат возвращается в виде текста. Вычисляется на стороне клиента в iframe canvas."
  },
  "TodoWrite": {
    "todos": "Полный текущий список задач (замена всего списка целиком)."
  },
  "llm_task": {
    "model": "Необязательное переопределение модели (например, 'anthropic/claude-sonnet-4-6'). По умолчанию используется настроенная модель по умолчанию.",
    "prompt": "Промпт для отправки в LLM.",
    "schema": "Необязательная JSON Schema для валидации ответа LLM. При указании LLM получает инструкцию вернуть корректный JSON, соответствующий этой схеме.",
    "temperature": "Необязательное переопределение temperature (0.0-2.0). По умолчанию используется настроенная temperature по умолчанию."
  },
  "browser_open": {
    "url": "URL HTTP или HTTPS для открытия в системном браузере"
  },
  "browser": {
    "action": "Действие браузера для выполнения (действия на уровне ОС требуют backend=computer_use)",
    "button": "Кнопка мыши для computer_use mouse_click",
    "by": "Для find: тип семантического локатора",
    "compact": "Для snapshot: удалять пустые структурные элементы",
    "depth": "Для snapshot: ограничить глубину дерева",
    "direction": "Направление прокрутки",
    "fill_value": "Для find с действием fill: значение для заполнения",
    "find_action": "Для find: действие, выполняемое над найденным элементом",
    "from_x": "X-координата источника перетаскивания (computer_use: mouse_drag)",
    "from_y": "Y-координата источника перетаскивания (computer_use: mouse_drag)",
    "full_page": "Для screenshot: захватить всю страницу",
    "interactive_only": "Для snapshot: показывать только интерактивные элементы",
    "key": "Клавиша для нажатия (Enter, Tab, Escape и т. д.)",
    "ms": "Количество миллисекунд ожидания",
    "path": "Путь к файлу для screenshot",
    "pixels": "Количество пикселей для прокрутки",
    "selector": "Селектор элемента: @ref (например, @e1), CSS (#id, .class) или text=...",
    "text": "Текст для ввода или ожидания",
    "to_x": "X-координата цели перетаскивания (computer_use: mouse_drag)",
    "to_y": "Y-координата цели перетаскивания (computer_use: mouse_drag)",
    "url": "URL для перехода (для действия 'open')",
    "value": "Значение для заполнения или ввода",
    "x": "X-координата экрана (computer_use: mouse_move/mouse_click)",
    "y": "Y-координата экрана (computer_use: mouse_move/mouse_click)"
  },
  "http_request": {
    "auth_secret": "Имя secret в [http_request.secrets] для отправки в качестве заголовка Authorization. Записи secret могут быть литеральными, зашифрованными или основанными на переменной окружения в виде ${ENV_VAR}. Переопределяет любой литеральный заголовок Authorization.",
    "body": "Необязательное тело запроса (для запросов POST, PUT, PATCH)",
    "headers": "Необязательные HTTP-заголовки в виде пар ключ-значение. Используйте auth_secret для значений Authorization, которые должны браться из config secrets.",
    "method": "HTTP-метод (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)",
    "url": "URL HTTP или HTTPS для запроса"
  },
  "web_fetch": {
    "url": "URL HTTP или HTTPS для получения"
  },
  "web_search_tool": {
    "query": "Поисковый запрос. Формулируйте конкретно для более точных результатов."
  },
  "backup": {
    "backup_name": "Имя резервной копии (для verify/restore)",
    "command": "Команда резервного копирования для выполнения",
    "confirm": "Подтвердить восстановление (обязательно для фактического восстановления, по умолчанию false)"
  },
  "screenshot": {
    "filename": "Необязательное имя файла (по умолчанию: screenshot_<timestamp>.png). Сохраняется в workspace.",
    "region": "Необязательная область для macOS: 'selection' для интерактивной обрезки, 'window' для активного окна. Игнорируется в Linux."
  },
  "image_info": {
    "path": "Путь к файлу изображения (абсолютный или относительный к workspace)"
  },
  "sessions_list": {
    "limit": "Максимальное количество сессий для возврата (по умолчанию: 50)"
  },
  "sessions_history": {
    "limit": "Максимальное количество сообщений для возврата, начиная с самых последних (по умолчанию: 20)",
    "session_id": "Session ID, из которого читается история (например, telegram__user123)"
  },
  "sessions_send": {
    "message": "Содержимое сообщения для отправки",
    "session_id": "Целевой session ID (например, telegram__user123). К сессиям Gateway dashboard можно обращаться по их dashboard ID или по gw_<id>."
  },
  "poll": {
    "channel": "Имя целевого канала. Если не указано, по умолчанию используется первый доступный канал.",
    "duration_minutes": "Длительность опроса в минутах (по умолчанию: 60)",
    "multi_select": "Разрешить множественный выбор (по умолчанию: false)",
    "options": "Варианты ответов опроса (от 2 до 10 пунктов)",
    "question": "Вопрос опроса",
    "recipient": "Идентификатор получателя/чата внутри канала (например, chat_id для Telegram, channel_id для Slack)"
  },
  "ask_user": {
    "channel": "Имя целевого канала. Если не указано, по умолчанию используется первый доступный канал.",
    "choices": "Необязательный список вариантов (отображается как кнопки в Telegram, пронумерованный список в CLI)",
    "question": "Вопрос для пользователя",
    "timeout_secs": "Количество секунд ожидания ответа (по умолчанию: 300)"
  },
  "reaction": {
    "action": "Добавить или удалить реакцию (по умолчанию: 'add')",
    "channel": "Имя канала, в котором добавляется реакция (например, 'discord', 'slack', 'telegram')",
    "channel_id": "Специфичный для платформы идентификатор канала/диалога (например, Discord channel snowflake, Slack channel ID)",
    "emoji": "Emoji для реакции (символ Unicode или platform shortcode)",
    "message_id": "Идентификатор сообщения в рамках платформы, на которое ставится реакция"
  },
  "git_forge": {
    "action": "'describe', 'raw' или действие над ресурсом (list/read/create/update/delete/add/remove/close/files/merge/request)",
    "base": "pull.create/update: целевая ветка",
    "body": "Текстовое тело для pull/issue/comment/review или сырой JSON payload для action=raw",
    "channel": "Ключ git-канала для маршрутизации (по умолчанию 'git')",
    "color": "label.create/update: 6-значный hex-цвет без '#'",
    "comment_id": "comment.delete: id комментария для удаления",
    "commit_message": "pull.merge: тело коммита squash/merge",
    "commit_title": "pull.merge: тема коммита squash/merge",
    "description": "Описание для milestone/label create/update",
    "draft": "pull.create: открыть как draft",
    "event": "review.create: APPROVE|REQUEST_CHANGES|COMMENT",
    "head": "pull.create: исходная ветка (или owner:branch для head форка)",
    "labels": "label.add: имена для присвоения; pull/issue.list: фильтр по label",
    "merge_method": "pull.merge: merge|squash|rebase",
    "method": "только для raw: HTTP-глагол GET|POST|PATCH|PUT|DELETE",
    "milestone": "issue.update: числовой milestone id для установки (номер PR является допустимым номером issue)",
    "name": "Имя label для label.create/update/delete/remove; new_name переименовывает",
    "new_name": "label.update: переименовать label в это значение",
    "number": "Номер issue или PR (для действий в рамках элемента)",
    "page": "действия list: номер страницы (нумерация с 1); увеличивайте, пока не получите неполную страницу, чтобы исчерпать результаты",
    "path": "только для raw: путь относительно провайдера, например repos/owner/repo/issues/12",
    "per_page": "действия list: размер страницы (максимум 100)",
    "repo": "Целевой репозиторий в формате 'owner/repo'",
    "resource": "Ресурс для типизированного вызова: milestone|label|issue|pull|review|reviewer|comment",
    "reviewers": "reviewer.request/remove: логины ревьюеров",
    "state": "open|closed; фильтрует действия list или задает state при update",
    "title": "Заголовок для pull.create, issue/milestone create/update"
  },
  "send_via": {
    "body": "Содержимое сообщения для немедленной рассылки (fanout). Опустите, чтобы вместо этого задать инструкцию маршрутизации для текущего ответа.",
    "modality": "Модальность доставки. Опустите, чтобы наследовать от output_modality группы peer.",
    "target": "Alias канала (например, telegram.default) или имя группы peer. Обязателен, если указан body. В остальных случаях необязателен (пропуск = тот же канал)."
  },
  "escalate_to_human": {
    "channel": "Канал для эскалации. По умолчанию канал, в котором происходит текущий диалог.",
    "context": "Подробный контекст для человека",
    "summary": "Краткое однострочное описание эскалации",
    "timeout_secs": "Количество секунд ожидания ответа, если wait_for_response равен true (по умолчанию: 600)",
    "urgency": "Уровень срочности (по умолчанию: medium). high/critical также уведомляет alert_channels.",
    "wait_for_response": "Блокировать выполнение и вернуть ответ человека (по умолчанию: false)"
  },
  "delegate": {
    "action": "Действие для выполнения. По умолчанию: 'delegate'. Используйте 'check_result' для получения результата фоновой задачи, 'await_sessions' для ожидания нескольких фоновых результатов, 'list_results' для вывода списка всех фоновых задач, 'cancel_task' для отмены выполняющейся фоновой задачи.",
    "agent": "Имя агента, которому делегируется задача. Доступно: (ничего не настроено)",
    "background": "Если true, sub-agent выполняется в фоновой задаче tokio и сразу возвращает task_id. Результаты сохраняются в workspace/delegate_results/{task_id}.json.",
    "context": "Необязательный контекст для добавления в начало (например, релевантный код, предыдущие находки)",
    "parallel": "Массив имен агентов для параллельного запуска с одним и тем же промптом. Возвращает все результаты после завершения всех агентов. Не может сочетаться с 'background'.",
    "prompt": "Задача/промпт для отправки sub-agent",
    "task_id": "Task ID для действий check_result/cancel_task (возвращается фоновым делегированием).",
    "task_ids": "Task ID для await_sessions.",
    "timeout_ms": "Максимальное количество миллисекунд ожидания для await_sessions перед возвратом частичных результатов. Ограничено значением 120000."
  },
  "execute_pipeline": {
    "parallel": "Выполнять шаги параллельно (без интерполяции). По умолчанию: false",
    "result": "Что возвращать: 'all' (по умолчанию) = результат каждого шага в виде JSON; 'last' = только сырой вывод последнего шага. Используйте 'last', чтобы не засорять контекст крупными промежуточными данными (например, base64).",
    "steps": "Упорядоченный список вызовов инструментов"
  }
};

/** tool name -> RU top-level description. */
export const toolDescriptionsRu: Record<string, string> = {
  "poll": "Создает опрос (poll) в канале обмена сообщениями. Для Telegram/Discord использует нативные опросы; для остальных каналов форматирует как пронумерованное текстовое сообщение с emoji-реакциями для голосования. На ACP-каналах, заявляющих elicitation.form, инструмент блокируется до выбора пользователя и возвращает JSON-строку результата с ключами `question`, `answer` (или `answers` для мультивыбора) и `channel`; в остальных случаях возвращает человекочитаемую строку подтверждения.",
  "reaction": "Добавляет или удаляет emoji-реакцию на сообщение в любом активном канале. Укажите имя канала (например, 'discord', 'slack'), platform channel ID, platform message ID и emoji (символ Unicode или platform shortcode)."
};

/** Locale-aware lookup for a tool argument's description. */
export function localizeToolArgDesc(tool: string, arg: string, en?: string): string | undefined {
  if (getLocale() === 'ru') { const ru = toolArgDescriptionsRu[tool]?.[arg]; if (ru) return ru; }
  return en;
}

/** Locale-aware lookup for a tool's top-level description. */
export function localizeToolDesc(tool: string, en?: string): string | undefined {
  if (getLocale() === 'ru') { const ru = toolDescriptionsRu[tool]; if (ru) return ru; }
  return en;
}
