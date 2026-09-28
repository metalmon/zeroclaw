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
cfg-err-peer-groups-agents-but-agents-channels = peer_groups.{$group_name}.agents[{$i}] = {$member_str} but agents.{$member_str}.channels has no {$needs_msg}
cfg-err-peer-groups-agents-but-agents-is = peer_groups.{$group_name}.agents[{$i}] = {$member_str} but agents.{$member_str} is not configured
cfg-err-peer-groups-channel-but-channels-is = peer_groups.{$group_name}.channel = {$group_channel} but [channels.{$channel_type}.{$alias}] is not configured
cfg-err-peer-groups-channel-but-no-channels = peer_groups.{$group_name}.channel = {$group_channel} but no [channels.{$channel_type}.*] block is configured
cfg-err-peer-groups-channel-but-no-channels-2 = peer_groups.{$group_name}.channel = {$group_channel} but no [channels.{$channel_type}.*] block is configured
cfg-err-peer-groups-channel-must-name-a = peer_groups.{$group_name}.channel must name a channel type (e.g. "discord") or dotted alias (e.g. "discord.work")
cfg-err-agents-workspace-read-memory-from-points = agents.{$alias}.workspace.read_memory_from[{$i}] points at agents.{$target_str} which uses memory backend {$target_backend}, but agents.{$alias} uses {$agent_backend}; the allowlist must point at same-backend siblings only
cfg-err-agents-workspace-read-memory-from-but = agents.{$alias}.workspace.read_memory_from[{$i}] = {$target_str} but agents.{$target_str} is not configured
cfg-err-agents-workspace-read-memory-from-but-2 = agents.{$alias}.workspace.read_memory_from[{$i}] = {$target_str} but {$target_str} is this agent itself; an agent always sees its own memory rows, so self-references in the cross-agent allowlist are not permitted
cfg-err-agents-workspace-access-but-agents-is = agents.{$alias}.workspace.access.{$target_str} = {$mode} but agents.{$target_str} is not configured
cfg-err-agents-workspace-access-but-is-this = agents.{$alias}.workspace.access.{$target_str} = {$mode} but {$target_str} is this agent itself; an agent always has full access to its own workspace, so self-references in the cross-agent allowlist are not permitted
cfg-err-agents-delegates-agent-duplicates-an-earlier = agents.{$alias}.delegates[{$i}].agent = {$target_str} duplicates an earlier delegate target
cfg-err-agents-delegates-agent-but-agents-is = agents.{$alias}.delegates[{$i}].agent = {$target_str} but agents.{$target_str} is not configured
cfg-err-agents-delegates-agent-names-this-agent = agents.{$alias}.delegates[{$i}].agent = {$target_str} names this agent itself; an agent cannot delegate to itself
cfg-err-agents-but-is-not-configured = agents.{$alias}.{$field} = {$trimmed} but {$section}.{$trimmed} is not configured
cfg-err-agents-but-is-not-configured-2 = agents.{$alias}.{$field}[{$i}] = {$trimmed} but {$section}.{$trimmed} is not configured
cfg-err-agents-must-be-dotted-form-type = agents.{$alias}.{$field} must be dotted form `<type>.<alias>` (got {$value})
cfg-err-agents-but-is-not-configured-3 = agents.{$alias}.{$field} = {$value} but {$section_prefix}.{$ty}.{$inner} is not configured
cfg-err-agents-channels-must-be-dotted-form = agents.{$alias}.channels[{$i}] must be dotted form `<type>.<alias>` (got {$trimmed})
cfg-err-agents-channels-but-channels-is-not = agents.{$alias}.channels[{$i}] = {$trimmed} but channels.{$ty}.{$inner} is not configured
cfg-err-agents-model-provider-must-be-dotted = agents.{$alias}.model_provider must be dotted form `<type>.<alias>` (got {$mp})
cfg-err-agents-model-provider-but-providers-models = agents.{$alias}.model_provider = {$mp} but [providers.models.{$ty}.{$inner}] is not configured
cfg-err-agents-model-provider-but-is-not = agents.{$alias}.model_provider = {$mp} but {$ty} is not a known provider family; check [providers.models.<family>.<alias>] in config.toml (valid families: `voltd providers`)
cfg-err-agents-model-provider-must-reference-a = agents.{$alias}.model_provider must reference a configured model model_provider (e.g. "anthropic.default")
cfg-err-runtime-profiles-context-compression-summary-provider = runtime_profiles.{$palias}.context_compression.summary_provider must be dotted form `<type>.<alias>` (got {$value})
cfg-err-runtime-profiles-context-compression-summary-provider-2 = runtime_profiles.{$palias}.context_compression.summary_provider = {$value} but providers.models.{$ty}.{$inner} is not configured
cfg-err-embedding-routes-model-provider-must-be = embedding_routes[{$i}].model_provider must be dotted form `<type>.<alias>` (got {$mp})
cfg-err-embedding-routes-model-provider-but-providers = embedding_routes[{$i}].model_provider = {$mp} but providers.models.{$ty}.{$inner} is not configured
cfg-err-model-routes-model-provider-must-be = model_routes[{$i}].model_provider must be dotted form `<type>.<alias>` (got {$mp})
cfg-err-model-routes-model-provider-but-providers = model_routes[{$i}].model_provider = {$mp} but providers.models.{$ty}.{$inner} is not configured
cfg-err-heartbeat-agent-but-no-agents-entry = heartbeat.agent = {$hb_agent} but no [agents.{$hb_agent}] entry is configured
cfg-err-true-requires-every-agent-on-the = {$flag_path} = true requires every agent on the sqlite memory backend (typed memory storage is SQLite-only), but agents.{$alias}.memory.backend = {$agent_backend}
cfg-err-http-request-secrets-key-must-contain = http_request.secrets key {$name} must contain 1..=64 ASCII letters, numbers, underscores, or hyphens
cfg-err-is-required-when-provider-set-the = {$path} is required when provider = "{$provider}": set the instance's API base URL including /api/v1 (e.g. https://git.example.org/api/v1); no default host is assumed because API requests carry the access token
cfg-err-flag-requires-sqlite-backend = {$flag_path} = true requires memory.backend = "sqlite" (typed memory storage is SQLite-only), but memory.backend = {$backend}
cfg-err-plugins-egress-not-granted = plugins.entries.{$entry_name}.egress_allow_private lists {$private}, which is not granted by egress_hosts; the carveout relaxes an address class for a granted destination, it does not grant one. A wildcard carveout ('*.host') needs an equal-or-broader wildcard grant, not an exact one
cfg-err-could-not-serialize-json-value = could not serialize JSON value: {$e}
cfg-err-float-field-requires-a-number-got = float field requires a number; got {$type}
cfg-err-integer-field-requires-a-whole-number = integer field requires a whole number; got {$type}
cfg-err-bool-field-requires-true-false-got = bool field requires `true`/`false`; got {$type}
cfg-err-object-field-requires-a-json-object = object field requires a JSON object; got {$type}
cfg-err-object-array-field-requires-a-json = object-array field requires a JSON array of objects; got {$type}
cfg-err-vec-string-field-requires-a-json = `Vec<String>` field requires a JSON array; got {$type}
cfg-err-array-element-is-vec-string-requires = array element [{$i}] is {$type} — `Vec<String>` requires string elements
