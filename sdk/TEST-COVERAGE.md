# SDK 验收与覆盖清单

对照 [sdk-test.md](../docs/sdk-test.md) 的全部 89 个稳定编号。前置条件、输入变体和预期错误沿用原测试计划；下表给出对应的实际入口。参数化函数覆盖同一条目的多个变体，攻击夹具先确认正常样本可验真。

验收环境：Windows、Rust 1.99.1。34 个运行测试、8 个独立配置进程场景、29 个编译失败 rustdoc、格式检查、Clippy `-D warnings` 及 rustdoc `-D warnings` 构建均通过；固定向量另经 Python 标准库独立哈希核对。三个角色正常依赖只指向 core，`cargo package --list` 已确认 verifier 包包含 fixtures、测试辅助模块和生成器。

服务层 SV-02～SV-07 为测试内存事务模型与调用契约单元测试，模型测试已通过。现有服务/钱包工程尚没有完整数据库、应用会话或浏览器交互实现，因此真实数据库隔离级别、HTTP Cookie 标志、加密存储及 UI 同意流程的部署集成验收不在本次 SDK 测试结果内。SDK 用例无未完成项；不能把模型结果当作这些外部组件已验收。

| 编号 | 测试文件 | 测试函数 / 场景 | 结果 |
|---|---|---|---|
| ID-01 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| ID-02 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| ID-03 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| ID-04 | [adversarial.rs](verifier/tests/adversarial.rs) | `id_04_07_fixed_secret_across_claims_and_issuers` | 通过 |
| ID-05 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| ID-06 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| ID-07 | [adversarial.rs](verifier/tests/adversarial.rs) | `id_04_07_fixed_secret_across_claims_and_issuers` | 通过 |
| ID-08 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| ID-09 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `id_01_02_03_05_06_08_09_stability_and_isolation` | 通过 |
| EN-01 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_01_offline_zero_one_three_claims` | 通过 |
| EN-02 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-03 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-04 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-05 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-06 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-07 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-08 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `en_02_03_04_05_06_07_08_enrollment_adversaries` | 通过 |
| EN-09 | [tests.rs](issuer/src/tests.rs) | `en_09_bk_01_05_organization_and_nonzero_keys` | 通过 |
| DS-01 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| DS-02 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| DS-03 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| DS-04 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| DS-05 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| DS-06 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| DS-07 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `ds_01_02_03_04_05_06_07_selection_contract` | 通过 |
| AT-01 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| AT-02 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| AT-03 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| AT-04 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| AT-05 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| AT-06 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_06_valid_bbs_proof_with_another_pseudonym_secret_is_rejected` | 通过 |
| AT-07 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| AT-08 | [adversarial.rs](verifier/tests/adversarial.rs) | `at_01_02_03_04_05_07_08_tampering_and_splicing` | 通过 |
| CH-01 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| CH-02 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| CH-03 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| CH-04 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| CH-05 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| CH-06 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| CH-07 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `ch_01_02_03_04_05_06_07_context_and_cookie_binding` | 通过 |
| TM-01 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-02 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-03 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-04 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-05 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-06 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-07 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| TM-08 | [challenge_time.rs](verifier/tests/challenge_time.rs) | `tm_01_02_03_04_05_06_07_08_both_roles_and_u64_boundaries` | 通过 |
| CF-01 | [tests.rs](core/src/tests.rs) | `cf_01_defaults_are_value_snapshots` | 通过 |
| CF-02 | [configuration.rs](verifier/tests/configuration.rs) | `main / child（small、large 两个独立进程）` | 通过 |
| CF-03 | [limits.rs](core/tests/limits.rs) | `main / run（六种独立进程配置）` | 通过 |
| CF-04 | [limits.rs](core/tests/limits.rs) | `main / run（六种独立进程配置）` | 通过 |
| CF-05 | [limits.rs](core/tests/limits.rs) | `main / run（六种独立进程配置）` | 通过 |
| CF-06 | [configuration.rs](verifier/tests/configuration.rs) | `main / child（small、large 两个独立进程）` | 通过 |
| CO-01 | [tests.rs](core/src/tests.rs) | `co_01_02_03_04_claim_semantics_and_duplicates` | 通过 |
| CO-02 | [tests.rs](core/src/tests.rs) | `co_01_02_03_04_claim_semantics_and_duplicates` | 通过 |
| CO-03 | [tests.rs](core/src/tests.rs) | `co_01_02_03_04_claim_semantics_and_duplicates` | 通过 |
| CO-04 | [tests.rs](core/src/tests.rs) | `co_01_02_03_04_claim_semantics_and_duplicates` | 通过 |
| CO-05 | [tests.rs](core/src/tests.rs) | `co_05_pv_03_parameters_prefix_and_independent_hashes` | 通过 |
| CO-06 | [tests.rs](core/src/tests.rs) | `co_06_strict_tokens_points_and_subgroups` | 通过 |
| CO-07 | [tests.rs](core/src/tests.rs) | `co_07_origin_normalization_and_rejection` | 通过 |
| CO-08 | [limits.rs](core/tests/limits.rs) | `main / run（六种独立进程配置）` | 通过 |
| WI-01 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_01_02_09_canonical_messages_hashes_and_mutable_dtos` | 通过 |
| WI-02 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_01_02_09_canonical_messages_hashes_and_mutable_dtos` | 通过 |
| WI-03 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_03_04_05_06_07_strict_json_and_bounded_proof_containers` | 通过 |
| WI-04 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_03_04_05_06_07_strict_json_and_bounded_proof_containers` | 通过 |
| WI-05 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_03_04_05_06_07_strict_json_and_bounded_proof_containers` | 通过 |
| WI-06 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_03_04_05_06_07_strict_json_and_bounded_proof_containers` | 通过 |
| WI-07 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_03_04_05_06_07_strict_json_and_bounded_proof_containers` | 通过 |
| WI-08 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_08_role_entry_points_revalidate_dtos` | 通过 |
| WI-09 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `wi_01_02_09_canonical_messages_hashes_and_mutable_dtos` | 通过 |
| BK-01 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `bk_01_02_03_04_05_restore_and_tampering` | 通过 |
| BK-02 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `bk_01_02_03_04_05_restore_and_tampering` | 通过 |
| BK-03 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `bk_01_02_03_04_05_restore_and_tampering` | 通过 |
| BK-04 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `bk_01_02_03_04_05_restore_and_tampering` | 通过 |
| BK-05 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `bk_01_02_03_04_05_restore_and_tampering` | 通过 |
| BK-06 | [backups_wire.rs](verifier/tests/backups_wire.rs) | `bk_06_logs_and_error_chains_are_redacted；另见 type_contracts.rs 的编译失败测试` | 通过 |
| PV-01 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `pv_01_02_and_sv_01_randomization_privacy_and_statelessness` | 通过 |
| PV-02 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `pv_01_02_and_sv_01_randomization_privacy_and_statelessness` | 通过 |
| PV-03 | [vectors.rs](verifier/tests/vectors.rs) | `pv_03_04_frozen_vectors_and_independent_message_encoding` | 通过 |
| PV-04 | [vectors.rs](verifier/tests/vectors.rs) | `pv_03_04_frozen_vectors_and_independent_message_encoding` | 通过 |
| PV-05 | [adversarial.rs](verifier/tests/adversarial.rs) | `pv_05_cross_purpose_and_context_reuse_fails` | 通过 |
| SV-01 | [offline_flow.rs](verifier/tests/offline_flow.rs) | `pv_01_02_and_sv_01_randomization_privacy_and_statelessness` | 通过 |
| SV-02 | [service_contracts.rs](verifier/tests/service_contracts.rs) | `sv_02_atomic_consume_has_one_winner_and_cookie_only_after_commit` | 通过（服务模型） |
| SV-03 | [service_contracts.rs](verifier/tests/service_contracts.rs) | `sv_03_04_expiry_rollback_missing_consumed_and_full_record_match` | 通过（服务模型） |
| SV-04 | [service_contracts.rs](verifier/tests/service_contracts.rs) | `sv_03_04_expiry_rollback_missing_consumed_and_full_record_match` | 通过（服务模型） |
| SV-05 | [service_contracts.rs](verifier/tests/service_contracts.rs) | `sv_05_issuance_ledger_idempotence_conflicts_concurrency_and_rollback` | 通过（服务模型） |
| SV-06 | [service_contracts.rs](verifier/tests/service_contracts.rs) | `sv_06_business_values_are_evaluated_after_cryptographic_verification` | 通过（服务模型） |
| SV-07 | [service_contracts.rs](verifier/tests/service_contracts.rs) | `sv_07_consent_trusted_origin_cookie_source_and_server_clock` | 通过（服务模型） |

补充覆盖：

- EN-05 的等价 JSON 输入还由 `wi_01_02_09_canonical_messages_hashes_and_mutable_dtos` 核对。
- CO-06 / AT-08 的非子群点由 core 测试用曲线点 `(0, 2)` 构造，并先断言其在曲线上且不属于素数阶子群。
- CO-08 / BK-05 的角色入口、wire 输出和备份资源限制另由 `verifier/tests/configuration.rs` 的小配置进程验证；无法在安全 API 下构造超限不可变 Claims，因此非法输入通过原始 JSON 和可变披露 DTO 注入。
- WI-04 的 `usize/u64` 及 `3+n` 溢出由 core `wi_04_counts_overflow_without_allocation` 补充，不分配伪造数量的元素。
- BK-06 另含 `verifier/src/type_contracts.rs` 的 29 个独立编译失败用例；SDK 控制缓冲区的擦除不通过读取已释放内存测试。
- holder、issuer、verifier 各有自己的单元测试；跨角色测试仅通过 verifier 的 dev-dependencies 引入另两个角色。
- PV-03 / CF-06 在不同配置进程比较规范消息、参数、请求哈希、组织指纹和 subject；固定向量是只读期望值，测试不会自动更新。
