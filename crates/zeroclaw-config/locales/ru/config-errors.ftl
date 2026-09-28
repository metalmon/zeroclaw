# Russian config-CRUD validation error messages.
# Keys not present here fall back to the English catalog in ../en/config-errors.ftl.

cfg-err-exchange-empty = поле exchange не должно быть пустым
cfg-err-plugins-limits-max-connections-per-instance = plugins.limits.max_connections_per_instance должно быть больше 0; нулевой предел отвергает все сетевые подключения плагинов
cfg-err-plugins-limits-call-timeout-ms-must = plugins.limits.call_timeout_ms должно быть больше 0; нулевой дедлайн прерывает каждый вызов плагина до его запуска
cfg-err-plugins-limits-max-instances-must-be = plugins.limits.max_instances должно быть больше 0; нулевой предел отвергает каждый плагин при создании
cfg-err-plugins-limits-max-table-elements-must = plugins.limits.max_table_elements должно быть больше 0; нулевой предел отвергает каждый плагин, выделяющий таблицу
cfg-err-plugins-limits-max-memory-mb-must = plugins.limits.max_memory_mb должно быть больше 0; нулевой лимит отвергает каждый плагин при создании
cfg-err-plugins-limits-call-fuel-must-be = plugins.limits.call_fuel должно быть больше 0; нулевой бюджет прерывает каждый вызов плагина до его запуска
cfg-err-plugins-max-active-instances-must-be = plugins.max_active_instances должно быть больше 0; нулевой предел отвергает каждый логический экземпляр плагина
cfg-err-agents-delegates-agent-is-empty-remove = agents.{$alias}.delegates[{$i}].agent пуст; удалите его или укажите настроенного агента
cfg-err-agents-risk-profile-must-reference-a = agents.{$alias}.risk_profile должен ссылаться на настроенную запись [risk_profiles.<alias>]
cfg-err-delegate-agentic-timeout-secs-must-be = delegate.agentic_timeout_secs должно быть больше 0
cfg-err-delegate-timeout-secs-must-be-greater = delegate.timeout_secs должно быть больше 0
cfg-err-notion-result-property-must-not-be = notion.result_property не должно быть пустым
cfg-err-notion-input-property-must-not-be = notion.input_property не должно быть пустым
cfg-err-notion-status-property-must-not-be = notion.status_property не должно быть пустым
cfg-err-notion-max-concurrent-must-be-greater = notion.max_concurrent должно быть больше 0
cfg-err-notion-poll-interval-secs-must-be = notion.poll_interval_secs должно быть больше 0
cfg-err-google-workspace-allowed-operations-methods-must = google_workspace.allowed_operations[{$i}].methods не должно быть пустым
cfg-err-google-workspace-allowed-operations-service-must = google_workspace.allowed_operations[{$i}].service не должно быть пустым
cfg-err-google-workspace-allowed-services-must-not = google_workspace.allowed_services[{$i}] не должно быть пустым
cfg-err-knowledge-db-path-must-not-be = knowledge.db_path не должно быть пустым
cfg-err-knowledge-max-nodes-must-be-greater = knowledge.max_nodes должно быть больше 0
cfg-err-embedding-routes-model-must-not-be = embedding_routes[{$i}].model не должно быть пустым
cfg-err-embedding-routes-model-provider-must-not = embedding_routes[{$i}].model_provider не должно быть пустым
cfg-err-embedding-routes-hint-must-not-be = embedding_routes[{$i}].hint не должно быть пустым
cfg-err-model-routes-model-must-not-be = model_routes[{$i}].model не должно быть пустым
cfg-err-model-routes-model-provider-must-not = model_routes[{$i}].model_provider не должно быть пустым
cfg-err-model-routes-hint-must-not-be = model_routes[{$i}].hint не должно быть пустым
cfg-err-scheduler-max-tasks-must-be-greater = scheduler.max_tasks должно быть больше 0
cfg-err-scheduler-max-concurrent-must-be-greater = scheduler.max_concurrent должно быть больше 0
cfg-err-security-leak-detection-sensitivity-must-be = security.leak_detection.sensitivity должно быть между 0.0 и 1.0
cfg-err-security-estop-state-file-must-not = security.estop.state_file не должно быть пустым
cfg-err-security-otp-gated-actions-must-not = security.otp.gated_actions[{$i}] не должно быть пустым
cfg-err-security-otp-challenge-max-attempts-must = security.otp.challenge_max_attempts должно быть больше 0
cfg-err-security-otp-cache-valid-secs-must = security.otp.cache_valid_secs должно быть больше 0
cfg-err-security-otp-token-ttl-secs-must = security.otp.token_ttl_secs должно быть больше 0
cfg-err-security-otp-challenge-max-attempts-must-2 = security.otp.challenge_max_attempts должно быть больше 0
cfg-err-msg = {$e}
cfg-err-msg-2 = {$e}
cfg-err-gateway-path-prefix-must-not-end = gateway.path_prefix не должно заканчиваться на «/» (включая одиночный «/»)
cfg-err-gateway-path-prefix-must-start-with = gateway.path_prefix должно начинаться с «/»
cfg-err-heartbeat-agent-must-reference-a-configured = heartbeat.agent должен ссылаться на настроенного агента, когда heartbeat.enabled = true
cfg-err-agents-precheck-timeout-secs-must-be = agents.{$alias}.precheck.timeout_secs должно быть больше 0
cfg-err-channels-max-concurrent-per-channel-must = channels.max_concurrent_per_channel должно быть больше 0
cfg-err-transcription-max-audio-bytes-must-be = transcription.max_audio_bytes должно быть больше нуля
cfg-err-nodes-mdns-peer-ttl-secs-must = nodes.mdns.peer_ttl_secs должно быть больше nodes.mdns.announce_interval_secs
cfg-err-nodes-mdns-peer-ttl-secs-must-2 = nodes.mdns.peer_ttl_secs должно быть больше 0
cfg-err-nodes-mdns-announce-interval-secs-must = nodes.mdns.announce_interval_secs должно быть больше 0
cfg-err-nodes-mdns-max-peers-must-be = nodes.mdns.max_peers должно быть больше 0
cfg-err-gateway-host-must-not-be-empty = gateway.host не должно быть пустым
cfg-err-is-out-of-range-must-be = {$path} = {$max_messages} вне диапазона; должно быть 0..={$MAX_SLACK_THREAD_CONTEXT_MAX_MESSAGES}
cfg-err-is-out-of-range-must-be-2 = {$path} = {$depth} вне диапазона; должно быть 0..={$REPLY_QUEUE_DEPTH_CEILING}
cfg-err-is-out-of-range-must-be-3 = {$path} = {$secs} вне диапазона; должно быть 0..={$REPLY_MIN_INTERVAL_MAX_SECS}
cfg-err-storage-lucid-store-timeout-ms-must = storage.lucid.{$alias}.store_timeout_ms должно быть больше 0
cfg-err-storage-lucid-recall-timeout-ms-must = storage.lucid.{$alias}.recall_timeout_ms должно быть больше 0
cfg-err-storage-lucid-binary-path-must-not = storage.lucid.{$alias}.binary_path не должно быть пустым
cfg-err-tunnel-openvpn-connect-timeout-secs-must = tunnel.openvpn.connect_timeout_secs должно быть больше 0
cfg-err-tunnel-openvpn-config-file-must-not = tunnel.openvpn.config_file не должно быть пустым
cfg-err-is-out-of-range-must-be-4 = {$path} = {$websocket_ping_interval_secs} вне диапазона; должно быть 0..={$GATEWAY_WEBSOCKET_PING_INTERVAL_MAX_SECS}
cfg-err-cloud-ops-supported-clouds-must-not = cloud_ops.supported_clouds[{$i}] не должно быть пустым
cfg-err-client-id-must-not-be-empty = client_id не должно быть пустым
cfg-err-is-required-when-true = {$field_path} обязательно, когда {$enabled_path} = true
cfg-err-must-not-contain-the-unset-display = {$field_path} не должно содержать заполнитель «не задано»
cfg-err-is-required-when-true-2 = {$field_path} обязательно, когда {$enabled_path} = true
cfg-err-must-not-contain-the-unset-display-2 = {$field_path} не должно содержать заполнитель «не задано»
cfg-err-must-include-a-host = {$field} должно включать хост
cfg-err-must-use-http-or-https = {$field} должно использовать http:// или https://
cfg-err-must-be-a-valid-url = {$field} должно быть корректным URL: {$err}
cfg-err-must-not-be-empty = {$field} не должно быть пустым
cfg-err-mcp-servers-tls-ca-cert-path = mcp.servers[{$i}].tls_ca_cert_path не должно быть пустым
cfg-err-mcp-servers-tool-timeout-secs-must = mcp.servers[{$i}].tool_timeout_secs должно быть больше 0
cfg-err-mcp-servers-name-must-not-be = mcp.servers[{$i}].name не должно быть пустым
cfg-err-is-invalid-cost-rates-must-be = {$path} = {$value} некорректно; ставки стоимости должны быть конечными и в диапазоне от 0 до {$max} USD за настроенную единицу
cfg-err-peer-groups-agents-but-agents-channels = peer_groups.{$group_name}.agents[{$i}] = {$member_str}, но у agents.{$member_str}.channels нет {$needs_msg}
cfg-err-peer-groups-agents-but-agents-is = peer_groups.{$group_name}.agents[{$i}] = {$member_str}, но agents.{$member_str} не настроено
cfg-err-peer-groups-channel-but-channels-is = peer_groups.{$group_name}.channel = {$group_channel}, но [channels.{$channel_type}.{$alias}] не настроено
cfg-err-peer-groups-channel-but-no-channels = peer_groups.{$group_name}.channel = {$group_channel}, но блок [channels.{$channel_type}.*] не настроен
cfg-err-peer-groups-channel-but-no-channels-2 = peer_groups.{$group_name}.channel = {$group_channel}, но блок [channels.{$channel_type}.*] не настроен
cfg-err-peer-groups-channel-must-name-a = peer_groups.{$group_name}.channel должно указывать тип канала (например, "discord") или точечный псевдоним (например, "discord.work")
cfg-err-agents-workspace-read-memory-from-points = agents.{$alias}.workspace.read_memory_from[{$i}] указывает на agents.{$target_str}, который использует бэкенд памяти {$target_backend}, а agents.{$alias} использует {$agent_backend}; список разрешений должен указывать только на соседей с тем же бэкендом
cfg-err-agents-workspace-read-memory-from-but = agents.{$alias}.workspace.read_memory_from[{$i}] = {$target_str}, но agents.{$target_str} не настроено
cfg-err-agents-workspace-read-memory-from-but-2 = agents.{$alias}.workspace.read_memory_from[{$i}] = {$target_str}, но {$target_str} — это сам этот агент; агент всегда видит свои строки памяти, поэтому ссылки на себя в межагентном списке разрешений не допускаются
cfg-err-agents-workspace-access-but-agents-is = agents.{$alias}.workspace.access.{$target_str} = {$mode}, но agents.{$target_str} не настроено
cfg-err-agents-workspace-access-but-is-this = agents.{$alias}.workspace.access.{$target_str} = {$mode}, но {$target_str} — это сам этот агент; агент всегда имеет полный доступ к своему рабочему пространству, поэтому ссылки на себя в межагентном списке разрешений не допускаются
cfg-err-agents-delegates-agent-duplicates-an-earlier = agents.{$alias}.delegates[{$i}].agent = {$target_str} дублирует ранее указанную цель делегирования
cfg-err-agents-delegates-agent-but-agents-is = agents.{$alias}.delegates[{$i}].agent = {$target_str}, но agents.{$target_str} не настроено
cfg-err-agents-delegates-agent-names-this-agent = agents.{$alias}.delegates[{$i}].agent = {$target_str} указывает на самого этого агента; агент не может делегировать сам себе
cfg-err-agents-but-is-not-configured = agents.{$alias}.{$field} = {$trimmed}, но {$section}.{$trimmed} не настроено
cfg-err-agents-but-is-not-configured-2 = agents.{$alias}.{$field}[{$i}] = {$trimmed}, но {$section}.{$trimmed} не настроено
cfg-err-agents-must-be-dotted-form-type = agents.{$alias}.{$field} должно быть в точечной форме `<type>.<alias>` (получено {$value})
cfg-err-agents-but-is-not-configured-3 = agents.{$alias}.{$field} = {$value}, но {$section_prefix}.{$ty}.{$inner} не настроено
cfg-err-agents-channels-must-be-dotted-form = agents.{$alias}.channels[{$i}] должно быть в точечной форме `<type>.<alias>` (получено {$trimmed})
cfg-err-agents-channels-but-channels-is-not = agents.{$alias}.channels[{$i}] = {$trimmed}, но channels.{$ty}.{$inner} не настроено
cfg-err-agents-model-provider-must-be-dotted = agents.{$alias}.model_provider должно быть в точечной форме `<type>.<alias>` (получено {$mp})
cfg-err-agents-model-provider-but-providers-models = agents.{$alias}.model_provider = {$mp}, но [providers.models.{$ty}.{$inner}] не настроено
cfg-err-agents-model-provider-but-is-not = agents.{$alias}.model_provider = {$mp}, но {$ty} не является известным семейством провайдеров; проверьте [providers.models.<family>.<alias>] в config.toml (допустимые семейства: `voltd providers`)
cfg-err-agents-model-provider-must-reference-a = agents.{$alias}.model_provider должно ссылаться на настроенный model_provider (например, "anthropic.default")
cfg-err-runtime-profiles-context-compression-summary-provider = runtime_profiles.{$palias}.context_compression.summary_provider должно быть в точечной форме `<type>.<alias>` (получено {$value})
cfg-err-runtime-profiles-context-compression-summary-provider-2 = runtime_profiles.{$palias}.context_compression.summary_provider = {$value}, но providers.models.{$ty}.{$inner} не настроено
cfg-err-embedding-routes-model-provider-must-be = embedding_routes[{$i}].model_provider должно быть в точечной форме `<type>.<alias>` (получено {$mp})
cfg-err-embedding-routes-model-provider-but-providers = embedding_routes[{$i}].model_provider = {$mp}, но providers.models.{$ty}.{$inner} не настроено
cfg-err-model-routes-model-provider-must-be = model_routes[{$i}].model_provider должно быть в точечной форме `<type>.<alias>` (получено {$mp})
cfg-err-model-routes-model-provider-but-providers = model_routes[{$i}].model_provider = {$mp}, но providers.models.{$ty}.{$inner} не настроено
cfg-err-heartbeat-agent-but-no-agents-entry = heartbeat.agent = {$hb_agent}, но запись [agents.{$hb_agent}] не настроена
cfg-err-true-requires-every-agent-on-the = {$flag_path} = true требует, чтобы каждый агент использовал бэкенд памяти sqlite (типизированное хранилище памяти только для SQLite), но agents.{$alias}.memory.backend = {$agent_backend}
cfg-err-http-request-secrets-key-must-contain = ключ http_request.secrets {$name} должен содержать 1..=64 ASCII-букв, цифр, подчеркиваний или дефисов
cfg-err-is-required-when-provider-set-the = {$path} обязательно, когда provider = "{$provider}": укажите базовый URL API инстанса, включая /api/v1 (например, https://git.example.org/api/v1); хост по умолчанию не подразумевается, так как запросы API несут токен доступа
cfg-err-flag-requires-sqlite-backend = {$flag_path} = true требует memory.backend = "sqlite" (типизированное хранилище памяти только для SQLite), но memory.backend = {$backend}
cfg-err-plugins-egress-not-granted = plugins.entries.{$entry_name}.egress_allow_private перечисляет {$private}, что не разрешено egress_hosts; послабление ослабляет класс адресов для уже разрешенного назначения, но не выдает разрешение. Wildcard-послабление ('*.host') требует такого же или более широкого wildcard-разрешения, а не точного
