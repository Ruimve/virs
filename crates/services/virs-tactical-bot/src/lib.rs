//! 策略基础设施 + 运行时引擎。
//!
//! 目录结构：
//! - [`prompt`]：策略 prompt 模板管理（文件加载 + 校验 + 占位符白名单 + 渲染）
//! - [`placeholder`]：占位符注册中心（单一数据源）
//! - [`llm_client`]：LLM HTTP 客户端封装
//! - [`engine`]：定时策略评估 + LLM 优化 + 热切换

pub mod engine;
pub mod llm_client;
pub mod placeholder;
pub mod prompt;
