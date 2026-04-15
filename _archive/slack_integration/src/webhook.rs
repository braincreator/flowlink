use anyhow::Result;
use hyper::{header, Body, Request, Response, Server, Method};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::*;

pub struct SlackWebhook {
    pub config: SlackWebhookConfig,
    pub server: Option<Server<hyper::body::Incoming, hyper::body::Incoming>>,
    pub is_running: Arc<RwLock<bool>>,
    pub command_handler: Arc<SlackCommandHandler>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SlackWebhookConfig {
    pub port: u16,
    pub signing_secret: String,
    pub verification_token: String,
}

impl SlackWebhook {
    pub async fn new(config: SlackWebhookConfig) -> Result<Self> {
        Ok(Self {
            config,
            server: None,
            is_running: Arc::new(RwLock::new(false)),
            command_handler: Arc::new(SlackCommandHandler::new(
                SlackConfig::default(), // TODO: pass proper config
                Arc::new(flowlink_relay::FlowLinkClient::new()),
            )),
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        *self.is_running.write().await = true;
        
        let port = self.config.port;
        let is_running = self.is_running.clone();
        let command_handler = self.command_handler.clone();
        let config = self.config.clone();
        
        let addr = format!("0.0.0.0:{}", port).parse().unwrap();
        
        let server = Server::bind(&addr).serve(|| {
            // TODO: Proper hyper service implementation
            hyper::service::make_service_fn(move |_conn| async {
                Ok::<_, hyper::Error>(hyper::service::service_fn(move |_req| {
                    async {
                        Response::builder()
                            .status(200)
                            .body(Body::from("OK"))
                    }
                }))
            })
        });
        
        self.server = Some(server);
        
        tokio::spawn(async move {
            log::info!("Slack webhook server started on port {}", port);
            
            if let Err(e) = server.await {
                log::error!("Slack webhook server error: {}", e);
            }
            
            *is_running.write().await = false;
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;
        
        // TODO: Properly shutdown the server
        if let Some(server) = self.server.take() {
            // This needs proper implementation
        }
        
        log::info!("Slack webhook server stopped");
        Ok(())
    }
    
    // Handle Slack events (URL verification, message events, etc.)
    pub async fn handle_slack_event(&self, req: Request<Body>) -> Result<Response<Body>> {
        let headers = req.headers();
        let method = req.method();
        
        // Handle URL verification
        if method == Method::GET && headers.get("X-Slack-Request-Timestamp").is_some() {
            return self.handle_url_verification(req).await;
        }
        
        // Handle events
        if method == Method::POST && headers.get("X-Slack-Signature").is_some() {
            return self.handle_event(req).await;
        }
        
        // Invalid request
        Ok(Response::builder()
            .status(400)
            .body(Body::from("Invalid request"))
            .unwrap())
    }
    
    async fn handle_url_verification(&self, req: Request<Body>) -> Result<Response<Body>> {
        // Parse query params
        let query = req.uri().query().unwrap_or("");
        let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
            .into_iter()
            .collect();
        
        if let Some(challenge) = params.get("challenge") {
            // Return the challenge token to verify the URL
            return Ok(Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from(challenge))
                .unwrap());
        }
        
        Ok(Response::builder()
            .status(400)
            .body(Body::from("Missing challenge parameter"))
            .unwrap())
    }
    
    async fn handle_event(&self, req: Request<Body>) -> Result<Response<Body>> {
        // TODO: Implement proper Slack event handling
        // 1. Verify the signature
        // 2. Parse the event
        // 3. Route to appropriate handler
        // 4. Send response
        
        let body = hyper::body::to_bytes(req.into_body()).await?;
        let event_body = String::from_utf8_lossy(&body);
        
        log::debug!("Received Slack event: {}", event_body);
        
        // Parse the event
        let slack_event: SlackEvent = serde_json::from_str(&event_body)?;
        
        match slack_event {
            SlackEvent::Message(message) => {
                self.handle_message_event(message).await?;
            }
            SlackEvent::EventCallback(event_callback) => {
                self.handle_event_callback(event_callback).await?;
            }
            _ => {
                log::debug!("Unhandled Slack event type");
            }
        }
        
        // Acknowledge receipt
        Ok(Response::builder()
            .status(200)
            .body(Body::from("OK"))
            .unwrap())
    }
    
    async fn handle_message_event(&self, message: SlackMessage) -> Result<()> {
        // Handle messages, commands, interactions
        if let Some(text) = &message.text {
            // Parse command
            if text.starts_with("!") {
                let parts: Vec<&str> = text[1..].split_whitespace().collect();
                if !parts.is_empty() {
                    let command = parts[0];
                    let args = parts[1..].to_vec();
                    
                    self.command_handler.handle_command(
                        message.channel,
                        message.user,
                        message.team,
                        command,
                        args,
                    ).await?;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_event_callback(&self, event_callback: SlackEventCallback) -> Result<()> {
        // Handle various event callbacks (app mentions, commands, etc.)
        match event_callback.kind {
            SlackEventCallbackType::AppMention => {
                self.handle_app_mention(event_callback).await?;
            }
            SlackEventCallbackType::InteractiveMessage => {
                self.handle_interactive_message(event_callback).await?;
            }
            _ => {
                log::debug!("Unhandled event callback type");
            }
        }
        
        Ok(())
    }
    
    async fn handle_app_mention(&self, event_callback: SlackEventCallback) -> Result<()> {
        // Handle app mentions (@flowlink command)
        if let Some(text) = &event_callback.event.text {
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() >= 2 {
                let command = parts[1];
                let args = parts[2..].to_vec();
                
                self.command_handler.handle_command(
                    event_callback.event.channel,
                    event_callback.event.user,
                    event_callback.team_id,
                    command,
                    args,
                ).await?;
            }
        }
        
        Ok(())
    }
    
    async fn handle_interactive_message(&self, event_callback: SlackEventCallback) -> Result<()> {
        // Handle interactive elements (buttons, select menus)
        if let Some(actions) = &event_callback.event.actions {
            for action in actions {
                match action.action_type.as_str() {
                    "button" => {
                        self.handle_button_press(event_callback.clone(), action).await?;
                    }
                    "select" => {
                        self.handle_select_option(event_callback.clone(), action).await?;
                    }
                    _ => {
                        log::debug!("Unknown action type: {}", action.action_type);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_button_press(&self, event_callback: SlackEventCallback, action: SlackInteractiveAction) -> Result<()> {
        // Handle button presses (approvals, etc.)
        if let Some(value) = &action.value {
            if value.starts_with("approve_") || value.starts_with("reject_") {
                let parts: Vec<&str> = value.split('_').collect();
                if parts.len() >= 3 {
                    let request_id = parts[1];
                    let action_type = parts[0];
                    
                    self.command_handler.handle_approval_action(
                        request_id,
                        action_type == "approve",
                        event_callback.event.channel,
                        event_callback.event.user,
                    ).await?;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_select_option(&self, event_callback: SlackEventCallback, action: SlackInteractiveAction) -> Result<()> {
        // Handle select menu choices
        log::debug!("Select option chosen: {:?}", action.selected_options);
        Ok(())
    }
}