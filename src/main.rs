#[macro_use]
extern crate rocket;

use rocket::form::Form;
use rocket::response::Redirect;
use rocket::serde::{Serialize, Deserialize};
use rocket::State;
use rocket::response::content::RawHtml;
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use std::time::{SystemTime, UNIX_EPOCH};
use html_escape::encode_text;

// Structure to represent a chat message
#[derive(Debug, Clone, FromForm)]
struct ChatMessage {
    username: String,
    message: String,
}

// Structure to hold the state of messages
#[derive(Serialize, Deserialize, Clone)]
struct Message {
    username: String,
    content: String,
}

// Shared state to hold all messages and rate-limiting info
struct ChatState {
    messages: Mutex<Vec<Message>>,
    user_request_timestamps: Mutex<HashMap<String, (u64, u64)>>, // To track request times and counts
    recent_messages: Mutex<HashSet<String>>, // To track recent unique messages
}

const REQUEST_LIMIT: u64 = 5; // Max requests in the time window
const TIME_WINDOW: u64 = 60; // Time window in seconds
const MAX_MESSAGE_LENGTH: usize = 200; // Maximum allowed message length
const RECENT_MESSAGE_LIMIT: usize = 100; // Limit for recent messages

// Function to check if a user is allowed to send a message
async fn is_request_allowed(username: &str, state: &ChatState) -> bool {
    let mut timestamps = state.user_request_timestamps.lock().await;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    if let Some((last_request_time, request_count)) = timestamps.get_mut(username) {
        if current_time - *last_request_time > TIME_WINDOW {
            *last_request_time = current_time;
            *request_count = 1;
            true
        } else if *request_count < REQUEST_LIMIT {
            *request_count += 1;
            true
        } else {
            false
        }
    } else {
        timestamps.insert(username.to_string(), (current_time, 1));
        true
    }
}

// Function to check if the message is valid (length and uniqueness)
async fn is_message_valid(message: &str, state: &ChatState) -> bool {
    if message.len() > MAX_MESSAGE_LENGTH {
        return false;
    }

    let mut recent_messages = state.recent_messages.lock().await;
    if recent_messages.contains(message) {
        return false;
    }

    if recent_messages.len() >= RECENT_MESSAGE_LIMIT {
        recent_messages.clear();
    }
    recent_messages.insert(message.to_string());

    true
}

// Index route to render chat interface
#[get("/?<username>")]
async fn index(username: Option<String>, state: &State<ChatState>) -> RawHtml<String> {
    let messages = state.messages.lock().await;

    let mut html = String::from(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
            <meta http-equiv="refresh" content="10"> <!-- Refresh every 10 seconds -->
            <title>AnonChat</title>
            <style>
                /* Reset basic styles for consistency across devices */
                * {
                    box-sizing: border-box;
                    margin: 0;
                    padding: 0;
                }

                /* Base styles */
                body {
                    background-color: #000000;
                    color: #e0e0e0;
                    font-family: Arial, sans-serif;
                    margin: 0;
                    padding: 0;
                    display: flex;
                    flex-direction: column;
                    min-height: 100vh;
                }

                /* Header */
                h1 {
                    font-size: 1.5em;
                    text-align: center;
                    color: #ffffff;
                    margin-bottom: 10px;
                }

                /* Chat container with overflow handling */
                #chat-container {
                    flex: 1;
                    background-color: #1e1e1e;
                    padding: 10px;
                    margin: 10px;
                    border-radius: 8px;
                    overflow-y: auto;
                    display: flex;
                    flex-direction: column;
                    max-height: 70vh;
                }

                #messages {
                    flex: 1; /* Allow messages to fill available space */
                    overflow-y: auto; /* Allow scrolling */
                }

                #messages p {
                    background-color: #2e2e2e;
                    border-left: 4px solid #00c853;
                    padding: 10px;
                    margin-bottom: 10px;
                    border-radius: 6px;
                    line-height: 1.5; /* Adjust line-height for better spacing */
                }

                /* Form container and responsive adjustments */
                #chat-form {
                    background-color: #1c1c1c;
                    padding: 10px;
                    border-radius: 8px;
                    width: 100%;
                    max-width: 600px;
                    margin: 0 auto;
                    box-shadow: 0 -4px 10px rgba(0, 0, 0, 0.5);
                }

                input[type="text"], input[type="submit"] {
                    border-radius: 6px;
                    padding: 10px;
                    margin-top: 5px;
                    width: 100%;
                    max-width: 100%;
                    background-color: #2e2e2e; /* Dark background for input field */
                    color: #e0e0e0; /* Text color for input field */
                    border: 1px solid #444; /* Optional: border for input field */
                }

                input[type="submit"] {
                    background-color: #007bff;
                    color: white;
                    border: none;
                    cursor: pointer;
                    transition: background-color 0.3s ease;
                }

                input[type="submit"]:hover {
                    background-color: #0056b3;
                }

                /* Media queries for smaller screens */
                @media (max-width: 768px) {
                    h1 {
                        font-size: 1.2em;
                    }

                    #chat-container {
                        max-height: 60vh;
                        margin: 5px;
                    }

                    #chat-form {
                        padding: 10px;
                    }

                    input[type="text"], input[type="submit"] {
                        font-size: 1em;
                        padding: 8px;
                    }
                }
            </style>
        </head>
        <body>
            <h1>AnonChat</h1>
            <div id="chat-container">
                <h2>Messages:</h2>
                <div id="messages">
        "#,
    );

    for msg in messages.iter() {
        html.push_str(&format!(
            "<p><strong>{}</strong>: {}</p>",
            encode_text(&msg.username),  // Escape username
            encode_text(&msg.content)     // Escape content
        ));
    }

    html.push_str(
        r#"
                </div>
            </div>
            <div id="chat-form">
                <form action="/send" method="post">
                    <label for="username">Username:</label>
                    <input type="text" id="username" name="username" required value="USERNAME_PLACEHOLDER"><br>
                    <label for="message">Message:</label>
                    <input type="text" id="message" name="message" required><br>
                    <input type="submit" value="Send">
                </form>
            </div>
        </body>
        </html>
        "#
    );

    let username_value = username.unwrap_or_else(|| "".to_string());
    let final_html = html.replace("USERNAME_PLACEHOLDER", &encode_text(&username_value)); // Escape username value

    RawHtml(final_html)
}

// Send message route with rate limiting, message length, and uniqueness check
#[post("/send", data = "<message_form>")]
async fn send(message_form: Form<ChatMessage>, state: &State<ChatState>) -> Result<Redirect, RawHtml<String>> {
    let username = encode_text(&message_form.username).to_string();
    let message = encode_text(&message_form.message).to_string();  

    if !is_request_allowed(&username, state.inner()).await {
        return Err(RawHtml("Too many requests. Please wait and try again.".to_string()));
    }

    if !is_message_valid(&message, state.inner()).await {
        return Err(RawHtml("Message is either too long or has already been sent.".to_string()));
    }

    sleep(Duration::from_secs(3)).await;

    let mut messages = state.messages.lock().await;
    messages.push(Message {
        username: username.clone(),
        content: message.clone(),
    });

    Ok(Redirect::to(format!("/?username={}", urlencoding::encode(&username))))
}

#[launch]
async fn rocket() -> _ {
    rocket::build()
        .manage(ChatState {
            messages: Mutex::new(vec![]),
            user_request_timestamps: Mutex::new(HashMap::new()),
            recent_messages: Mutex::new(HashSet::new()),
        })
        .mount("/", routes![index, send])
}
