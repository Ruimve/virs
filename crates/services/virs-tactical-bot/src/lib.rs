//! 策略基础设施层：被 auto bot 使用。
//!
//! 目录结构：
//! - [`prompt`]：策略 prompt 模板管理（文件加载 + 校验 + 占位符白名单 + 渲染）
//! - [`placeholder`]：占位符注册中心（单一数据源）
//! - [`llm_client`]：LLM HTTP 客户端封装

pub mod llm_client;
pub mod placeholder;
pub mod prompt;
