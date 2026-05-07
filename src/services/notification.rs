//! Notification service — Email and Telegram notifications.
//!
//! Sends alerts for trading signals, order executions, and errors.

use crate::config::NotificationConfig;
use tracing::{info, error};

/// Send a notification through all configured channels.
pub async fn send_notification(
    config: &NotificationConfig,
    title: &str,
    message: &str,
) {
    // Telegram
    if let Some(ref tg) = config.telegram {
        if let Err(e) = send_telegram(tg, title, message).await {
            error!("Telegram notification failed: {}", e);
        }
    }

    // Email
    if let Some(ref email) = config.email {
        if let Err(e) = send_email(email, title, message).await {
            error!("Email notification failed: {}", e);
        }
    }
}

/// Send a Telegram message.
async fn send_telegram(
    config: &crate::config::TelegramConfig,
    title: &str,
    message: &str,
) -> Result<(), String> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.bot_token
    );

    let text = format!("*{}*\n\n{}", title, message);

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": config.chat_id,
            "text": text,
            "parse_mode": "Markdown",
        }))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Telegram API error {}: {}", status, body));
    }

    info!("Telegram notification sent: {}", title);
    Ok(())
}

/// Send an email notification.
#[cfg(feature = "notification-email")]
async fn send_email(
    config: &crate::config::EmailConfig,
    title: &str,
    message: &str,
) -> Result<(), String> {
    use lettre::{Message, SmtpTransport, Transport};

    let email = Message::builder()
        .from(config.from.parse().map_err(|e| format!("Invalid from: {}", e))?)
        .to(config.from.parse().map_err(|e| format!("Invalid to: {}", e))?)
        .subject(title)
        .body(message.to_string())
        .map_err(|e| format!("Email build error: {}", e))?;

    let transport = SmtpTransport::starttls_relay(&config.host)
        .map_err(|e| format!("SMTP connect error: {}", e))?
        .port(config.port)
        .credentials(lettre::transport::smtp::authentication::Credentials::new(
            config.username.clone(),
            config.password.clone(),
        ))
        .build();

    transport
        .send(&email)
        .map_err(|e| format!("Send error: {}", e))?;

    info!("Email notification sent: {}", title);
    Ok(())
}

/// No-op when email feature is disabled.
#[cfg(not(feature = "notification-email"))]
async fn send_email(
    _config: &crate::config::EmailConfig,
    _title: &str,
    _message: &str,
) -> Result<(), String> {
    warn!("Email notification skipped: notification-email feature not enabled");
    Ok(())
}
