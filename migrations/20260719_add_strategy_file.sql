-- 为 qd_auto_bots 和 qd_grid_bots 添加 strategy_file 列
-- 用于支持文件化策略 prompt（STRATEGIES_DIR 环境变量指向的目录）
-- 对应代码：virs-types/src/auto_port.rs 和 grid_port.rs 的 strategy_file: Option<String> 字段
-- 加载逻辑：worker 优先查 strategies/{auto,grid}/{strategy_file}.json，未设置或未命中时回退到 DEFAULT_* 常量

ALTER TABLE qd_grid_bots
    ADD COLUMN IF NOT EXISTS strategy_file TEXT;

ALTER TABLE qd_auto_bots
    ADD COLUMN IF NOT EXISTS strategy_file TEXT;
