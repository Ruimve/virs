# virs-database 测试计划

## 概述

virs-database 是工作区唯一的数据访问层，负责数据库连接池管理、迁移执行和所有 SQL 查询。

## 测试清单

| 编号 | 测试函数名 | 场景 | 输入数据 | 预期结果 |
|------|-----------|------|---------|---------|
| 1 | test_create_pool | 创建有效连接池 | 有效 DATABASE_URL | 返回 PgPool，可执行查询 |
| 2 | test_create_pool_invalid_url | 无效连接字符串 | 无效 URL | 返回 VirsError |
| 3 | test_run_migrations | 执行迁移 | 空数据库 + PgPool | 所有表创建成功，迁移版本记录写入 |
| 4 | test_run_migrations_idempotent | 重复执行迁移 | 已迁移的 PgPool | 无操作，不报错 |
| 5 | test_ensure_admin_new | 首次创建管理员 | 不存在的用户名 | 插入成功，返回行数 1 |
| 6 | test_ensure_admin_existing | 管理员已存在 | 已存在的用户名 | 不插入，返回行数 0 |
| 7 | test_create_user | 创建新用户 | 合法用户名+密码哈希 | 返回用户 ID |
| 8 | test_find_user_by_username | 按用户名查询 | 已存在的用户名 | 返回用户信息 |
| 9 | test_find_user_not_found | 查询不存在用户 | 不存在的用户名 | 返回 None |
| 10 | test_insert_bot | 插入机器人 | 合法参数 | 数据库中可查询到记录 |
| 11 | test_get_bot_by_id | 按ID查询机器人 | 已插入的 bot ID | 返回 bot 配置 |
| 12 | test_verify_bot_ownership | 验证用户拥有机器人 | 正确的 user_id + bot_id | 返回 true |
| 13 | test_verify_bot_ownership_wrong_user | 验证错误用户 | 错误的 user_id | 返回 false |
| 14 | test_count_bots_by_user | 统计用户机器人数量 | 已有 N 个机器人的用户 | 返回 N |
| 15 | test_list_bots_by_user | 列出用户机器人 | 已有机器人的用户 | 返回机器人列表 |
| 16 | test_update_bot_strategy | 更新策略文件 | 新策略文件路径 | 数据库中 strategy_file 已更新 |
| 17 | test_save_ai_credential | 保存AI凭据 | 加密后的 API key | 数据库中可查询到记录 |
| 18 | test_list_ai_credentials | 列出AI凭据 | 已有凭据的用户 | 返回凭据列表 |
| 19 | test_delete_ai_credential | 删除AI凭据 | 已存在的凭据 ID | 记录已删除 |
| 20 | test_save_exchange_credential | 保存交易所凭据 | 加密后的 key/secret | 数据库中可查询到记录 |
| 21 | test_get_all_exchange_credentials | 查询所有交易所凭据 | 已有多条凭据 | 返回全部记录 |
| 22 | test_fetch_stop_loss_take_profit | 查询止损止盈价 | 有持仓的 symbol | 返回止损止盈价格 |
| 23 | test_fetch_stop_loss_take_profit_none | 无止损止盈记录 | 无持仓的 symbol | 返回 (None, None) |
| 24 | test_load_ai_credentials_for_bot | 查询机器人AI凭据 | 有凭据的用户 | 返回加密凭据列表 |
| 25 | test_get_latest_llm_credential | 查询最新LLM凭据 | 已有凭据 | 返回最新一条 |
| 26 | test_count_bot_trades | 统计机器人交易数 | 有交易的 bot | 返回交易总数 |
| 27 | test_query_bot_trades | 分页查询交易记录 | bot_id + 分页参数 | 返回对应页的交易列表 |
| 28 | test_get_bot_trade_stats | 查询交易统计 | 有交易的 bot | 返回统计数据 |
| 29 | test_count_analysis_logs | 统计分析日志数 | 有日志的 bot | 返回日志总数 |
| 30 | test_query_analysis_logs | 分页查询分析日志 | bot_id + 分页参数 | 返回对应页的日志列表 |
