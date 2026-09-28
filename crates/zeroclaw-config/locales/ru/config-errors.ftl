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
