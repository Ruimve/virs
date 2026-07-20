-- 拆分 qd_auto_bots.position_id 为 per-side 字段（Hedge 模式支持多空并存）
ALTER TABLE qd_auto_bots ADD COLUMN IF NOT EXISTS position_id_long UUID;
ALTER TABLE qd_auto_bots ADD COLUMN IF NOT EXISTS position_id_short UUID;
-- 迁移旧数据：旧 position_id 复制到两列（无法区分方向，两列都填）
UPDATE qd_auto_bots SET position_id_long = position_id, position_id_short = position_id WHERE position_id IS NOT NULL;
-- 删除旧字段
ALTER TABLE qd_auto_bots DROP COLUMN IF EXISTS position_id;

-- qd_grid_bots 不需要改（grid worker 不使用 position_id）
