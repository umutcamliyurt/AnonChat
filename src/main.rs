#[macro_use]
extern crate rocket;

use rocket::form::Form;
use rocket::response::Redirect;
use rocket::serde::{Serialize, Deserialize};
use rocket::State;
use rocket::response::content::RawHtml;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use std::time::Duration;

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
const TIME_WINDOW: u64 = 60; // Time window in seconds (e.g., 60 seconds)
const MAX_MESSAGE_LENGTH: usize = 200; // Maximum allowed message length
const RECENT_MESSAGE_LIMIT: usize = 100; // Limit the number of recent messages we track

// Function to check if a user is allowed to send a message
fn is_request_allowed(username: &str, state: &ChatState) -> bool {
    let mut timestamps = state.user_request_timestamps.lock().unwrap();
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Check if the user exists in the map
    if let Some((last_request_time, request_count)) = timestamps.get_mut(username) {
        if current_time - *last_request_time > TIME_WINDOW {
            // Reset the count if the time window has passed
            *last_request_time = current_time;
            *request_count = 1;
            true
        } else if *request_count < REQUEST_LIMIT {
            // Allow request if the count is within the limit
            *request_count += 1;
            true
        } else {
            // Deny the request as the user exceeded the limit
            false
        }
    } else {
        // First request from the user
        timestamps.insert(username.to_string(), (current_time, 1));
        true
    }
}

// Function to check if the message is valid (length and uniqueness)
fn is_message_valid(message: &str, state: &ChatState) -> bool {
    // Check for message length
    if message.len() > MAX_MESSAGE_LENGTH {
        return false;
    }

    // Check if the message is unique
    let mut recent_messages = state.recent_messages.lock().unwrap();
    if recent_messages.contains(message) {
        return false;
    }

    // Message is unique, so add it to the recent messages
    if recent_messages.len() >= RECENT_MESSAGE_LIMIT {
        // If the set is full, remove the oldest message (for simplicity, we clear the set)
        recent_messages.clear();
    }
    recent_messages.insert(message.to_string());

    true
}

// Index route to render chat interface
#[get("/?<username>")]
fn index(username: Option<String>, state: &State<ChatState>) -> RawHtml<String> {
    let messages = state.messages.lock().unwrap();

    // Render the HTML with all the chat messages
    let mut html = String::from(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <meta http-equiv="refresh" content="10"> <!-- Refresh the page every 10 seconds -->
            <title>AnonChat</title>
            <style>
                body {
                    background-color: #000000;
                    color: #e0e0e0;
                    font-family: Arial, sans-serif;
                    margin: 0;
                    padding: 20px;
                    display: flex;
                    flex-direction: column;
                    height: 100vh;
                }
                h1, h2 {
                    color: #ffffff;
                }
                #chat-container {
                    flex: 1;
                    background-color: #1e1e1e;
                    padding: 15px;
                    border-radius: 8px;
                    margin-bottom: 20px;
                    overflow-y: auto;
                }
                #messages p {
                    background-color: #2e2e2e;
                    border-left: 4px solid #00c853;
                    padding: 10px;
                    margin-bottom: 10px;
                    border-radius: 6px;
                }
                #chat-form {
                    background-color: #1c1c1c;
                    padding: 15px;
                    border-radius: 8px;
                    position: fixed;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                }
                input[type="text"] {
                    border-radius: 6px;
                    padding: 10px;
                    margin-top: 5px;
                    width: 100%;
                    background-color: #333;
                    color: white;
                    border: 1px solid #555;
                }
                input[type="submit"] {
                    background-color: #007bff;
                    color: white;
                    border: none;
                    cursor: pointer;
                    border-radius: 6px;
                    padding: 10px;
                    margin-top: 5px;
                    width: 100%;
                }
                input[type="submit"]:hover {
                    background-color: #0056b3;
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

    // Add each message to the HTML
    for msg in messages.iter() {
        html.push_str(&format!(
            "<p><strong>{}</strong>: {}</p>",
            msg.username, msg.content
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
    let final_html = html.replace("USERNAME_PLACEHOLDER", &username_value);

    RawHtml(final_html)  // Return the rendered HTML
}


// Send message route with rate limiting, message length, and uniqueness check
#[post("/send", data = "<message_form>")]
fn send(message_form: Form<ChatMessage>, state: &State<ChatState>) -> Result<Redirect, RawHtml<String>> {
    // Check if the request is allowed (rate limiting)
    if !is_request_allowed(&message_form.username, state.inner()) {
        return Err(RawHtml("Too many requests. Please wait and try again.".to_string()));
    }

    // Check if the message is valid (length and uniqueness)
    if !is_message_valid(&message_form.message, state.inner()) {
        return Err(RawHtml("Message is either too long or has already been sent.".to_string()));
    }

    // Introduce a delay of 3 seconds before processing the message
    thread::sleep(Duration::from_secs(3));

    let mut messages = state.messages.lock().unwrap();
    
    // Add the new message to the state
    messages.push(Message {
        username: message_form.username.clone(),
        content: message_form.message.clone(),
    });

    // Redirect back to the index page with the username
    Ok(Redirect::to(format!("/?username={}", urlencoding::encode(&message_form.username))))
}

// Launch the Rocket application
#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(ChatState {
            messages: Mutex::new(Vec::new()),
            user_request_timestamps: Mutex::new(HashMap::new()), // Initialize empty timestamp map
            recent_messages: Mutex::new(HashSet::new()), // Initialize empty set for recent messages
        })
        .mount("/", routes![index, send])
}
