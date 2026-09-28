# English config-CRUD validation error messages (source catalog).
# Keys are localized in the process locale via `zeroclaw_config::i18n::localize`
# (see the `validation_bail_i18n!` macro). Keys absent from a locale fall back
# to this English catalog.

cfg-err-exchange-empty = exchange must not be empty
cfg-err-plugins-limits-max-connections-per-instance = plugins.limits.max_connections_per_instance must be greater than 0; a zero ceiling rejects every plugin network connection
cfg-err-plugins-limits-call-timeout-ms-must = plugins.limits.call_timeout_ms must be greater than 0; a zero deadline aborts every plugin call before it runs
cfg-err-plugins-limits-max-instances-must-be = plugins.limits.max_instances must be greater than 0; a zero ceiling rejects every plugin at instantiation
cfg-err-plugins-limits-max-table-elements-must = plugins.limits.max_table_elements must be greater than 0; a zero ceiling rejects every plugin that allocates a table
cfg-err-plugins-limits-max-memory-mb-must = plugins.limits.max_memory_mb must be greater than 0; a zero cap rejects every plugin at instantiation
cfg-err-plugins-limits-call-fuel-must-be = plugins.limits.call_fuel must be greater than 0; a zero budget traps every plugin call before it runs
cfg-err-plugins-max-active-instances-must-be = plugins.max_active_instances must be greater than 0; a zero ceiling rejects every logical plugin instance
cfg-err-agents-delegates-agent-is-empty-remove = agents.{$alias}.delegates[{$i}].agent is empty; remove it or name a configured agent
cfg-err-agents-risk-profile-must-reference-a = agents.{$alias}.risk_profile must reference a configured [risk_profiles.<alias>] entry
cfg-err-delegate-agentic-timeout-secs-must-be = delegate.agentic_timeout_secs must be greater than 0
cfg-err-delegate-timeout-secs-must-be-greater = delegate.timeout_secs must be greater than 0
cfg-err-notion-result-property-must-not-be = notion.result_property must not be empty
cfg-err-notion-input-property-must-not-be = notion.input_property must not be empty
cfg-err-notion-status-property-must-not-be = notion.status_property must not be empty
cfg-err-notion-max-concurrent-must-be-greater = notion.max_concurrent must be greater than 0
cfg-err-notion-poll-interval-secs-must-be = notion.poll_interval_secs must be greater than 0
cfg-err-google-workspace-allowed-operations-methods-must = google_workspace.allowed_operations[{$i}].methods must not be empty
cfg-err-google-workspace-allowed-operations-service-must = google_workspace.allowed_operations[{$i}].service must not be empty
cfg-err-google-workspace-allowed-services-must-not = google_workspace.allowed_services[{$i}] must not be empty
cfg-err-knowledge-db-path-must-not-be = knowledge.db_path must not be empty
cfg-err-knowledge-max-nodes-must-be-greater = knowledge.max_nodes must be greater than 0
cfg-err-embedding-routes-model-must-not-be = embedding_routes[{$i}].model must not be empty
cfg-err-embedding-routes-model-provider-must-not = embedding_routes[{$i}].model_provider must not be empty
cfg-err-embedding-routes-hint-must-not-be = embedding_routes[{$i}].hint must not be empty
cfg-err-model-routes-model-must-not-be = model_routes[{$i}].model must not be empty
cfg-err-model-routes-model-provider-must-not = model_routes[{$i}].model_provider must not be empty
cfg-err-model-routes-hint-must-not-be = model_routes[{$i}].hint must not be empty
cfg-err-scheduler-max-tasks-must-be-greater = scheduler.max_tasks must be greater than 0
cfg-err-scheduler-max-concurrent-must-be-greater = scheduler.max_concurrent must be greater than 0
cfg-err-security-leak-detection-sensitivity-must-be = security.leak_detection.sensitivity must be between 0.0 and 1.0
cfg-err-security-estop-state-file-must-not = security.estop.state_file must not be empty
cfg-err-security-otp-gated-actions-must-not = security.otp.gated_actions[{$i}] must not be empty
cfg-err-security-otp-challenge-max-attempts-must = security.otp.challenge_max_attempts must be greater than 0
cfg-err-security-otp-cache-valid-secs-must = security.otp.cache_valid_secs must be greater than 0
cfg-err-security-otp-token-ttl-secs-must = security.otp.token_ttl_secs must be greater than 0
cfg-err-security-otp-challenge-max-attempts-must-2 = security.otp.challenge_max_attempts must be greater than 0
cfg-err-msg = {$e}
cfg-err-msg-2 = {$e}
cfg-err-gateway-path-prefix-must-not-end = gateway.path_prefix must not end with '/' (including bare '/')
cfg-err-gateway-path-prefix-must-start-with = gateway.path_prefix must start with '/'
cfg-err-heartbeat-agent-must-reference-a-configured = heartbeat.agent must reference a configured agent when heartbeat.enabled = true
cfg-err-agents-precheck-timeout-secs-must-be = agents.{$alias}.precheck.timeout_secs must be greater than 0
cfg-err-channels-max-concurrent-per-channel-must = channels.max_concurrent_per_channel must be greater than 0
cfg-err-transcription-max-audio-bytes-must-be = transcription.max_audio_bytes must be greater than zero
cfg-err-nodes-mdns-peer-ttl-secs-must = nodes.mdns.peer_ttl_secs must be greater than nodes.mdns.announce_interval_secs
cfg-err-nodes-mdns-peer-ttl-secs-must-2 = nodes.mdns.peer_ttl_secs must be greater than 0
cfg-err-nodes-mdns-announce-interval-secs-must = nodes.mdns.announce_interval_secs must be greater than 0
cfg-err-nodes-mdns-max-peers-must-be = nodes.mdns.max_peers must be greater than 0
cfg-err-gateway-host-must-not-be-empty = gateway.host must not be empty
cfg-err-is-out-of-range-must-be = {$path} = {$max_messages} is out of range; must be 0..={$MAX_SLACK_THREAD_CONTEXT_MAX_MESSAGES}
cfg-err-is-out-of-range-must-be-2 = {$path} = {$depth} is out of range; must be 0..={$REPLY_QUEUE_DEPTH_CEILING}
cfg-err-is-out-of-range-must-be-3 = {$path} = {$secs} is out of range; must be 0..={$REPLY_MIN_INTERVAL_MAX_SECS}
cfg-err-storage-lucid-store-timeout-ms-must = storage.lucid.{$alias}.store_timeout_ms must be greater than 0
cfg-err-storage-lucid-recall-timeout-ms-must = storage.lucid.{$alias}.recall_timeout_ms must be greater than 0
cfg-err-storage-lucid-binary-path-must-not = storage.lucid.{$alias}.binary_path must not be empty
cfg-err-tunnel-openvpn-connect-timeout-secs-must = tunnel.openvpn.connect_timeout_secs must be greater than 0
cfg-err-tunnel-openvpn-config-file-must-not = tunnel.openvpn.config_file must not be empty
cfg-err-is-out-of-range-must-be-4 = {$path} = {$websocket_ping_interval_secs} is out of range; must be 0..={$GATEWAY_WEBSOCKET_PING_INTERVAL_MAX_SECS}
cfg-err-cloud-ops-supported-clouds-must-not = cloud_ops.supported_clouds[{$i}] must not be empty
cfg-err-client-id-must-not-be-empty = client_id must not be empty
cfg-err-is-required-when-true = {$field_path} is required when {$enabled_path} = true
cfg-err-must-not-contain-the-unset-display = {$field_path} must not contain the unset display placeholder
cfg-err-is-required-when-true-2 = {$field_path} is required when {$enabled_path} = true
cfg-err-must-not-contain-the-unset-display-2 = {$field_path} must not contain the unset display placeholder
cfg-err-must-include-a-host = {$field} must include a host
cfg-err-must-use-http-or-https = {$field} must use http:// or https://
cfg-err-must-be-a-valid-url = {$field} must be a valid URL: {$err}
cfg-err-must-not-be-empty = {$field} must not be empty
cfg-err-mcp-servers-tls-ca-cert-path = mcp.servers[{$i}].tls_ca_cert_path must not be empty
cfg-err-mcp-servers-tool-timeout-secs-must = mcp.servers[{$i}].tool_timeout_secs must be greater than 0
cfg-err-mcp-servers-name-must-not-be = mcp.servers[{$i}].name must not be empty
cfg-err-is-invalid-cost-rates-must-be = {$path} = {$value} is invalid; cost rates must be finite and between 0 and {$max} USD per configured unit
