mod bot_repo;
mod credential_repo;
mod order_repo;
mod trade_repo;
mod user_repo;

pub use bot_repo::PgBotStore;
pub use bot_repo::{bot_to_config, count_all_bots, get_running_bot_symbols, get_running_paper_modes, mark_running_bots_as_error};
pub use bot_repo::{count_analysis_logs, count_bot_trades, count_bots_by_user, get_bot_by_id, get_bot_trade_stats, insert_bot, insert_strategy_selection_log, list_bots_by_user, query_analysis_logs, query_bot_trades, update_bot_strategy, verify_bot_ownership};
pub use credential_repo::{get_ai_providers, get_all_exchange_credentials, get_default_ai_credential, get_latest_llm_credential, get_user_exchange, list_ai_credentials, list_exchange_credentials, load_ai_credentials_for_bot, save_ai_credential, save_exchange_credential, delete_ai_credential, delete_exchange_credential};
pub use order_repo::PgOrderPersistence;
pub use order_repo::fetch_stop_loss_take_profit;
pub use trade_repo::PgTradeHistoryProvider;
pub use user_repo::{create_user, delete_user, ensure_admin, find_user_by_username, get_user_info, list_users, update_user};
