-- 为 pe_auto_order_context / pe_grid_order_context 和分析日志表添加 strategy_file 列
-- 用于行级快照：每笔交易/每次决策在 INSERT 时冻结当时使用的策略名
-- 与 qd_auto_bots.strategy_file / qd_grid_bots.strategy_file（可变，反映当前策略）配合：
--   bot 表 = 当前生效策略（worker 运行时读取）
--   context 表 = 交易发生时的策略（盈亏归因，永不 UPDATE）
--   logs 表 = 决策发生时的策略（决策质量分析，永不 UPDATE）

ALTER TABLE pe_auto_order_context
    ADD COLUMN IF NOT EXISTS strategy_file TEXT;

ALTER TABLE qd_auto_analysis_logs
    ADD COLUMN IF NOT EXISTS strategy_file TEXT;

ALTER TABLE pe_grid_order_context
    ADD COLUMN IF NOT EXISTS strategy_file TEXT;

ALTER TABLE qd_grid_analysis_logs
    ADD COLUMN IF NOT EXISTS strategy_file TEXT;
