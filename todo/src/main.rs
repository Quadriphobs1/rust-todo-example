#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;

use rocket::http::RawStr;
use rocket::request::FromFormValue;
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Mutex;

type ID = usize;

#[derive(Serialize, Deserialize, Copy, Clone)]
struct Priority(usize);

impl<'v> FromFormValue<'v> for Priority {
    type Error = &'v RawStr;

    fn from_form_value(form_value: &'v RawStr) -> Result<Priority, &'v RawStr> {
        match form_value.parse::<usize>() {
            Ok(data) if data >= 1 && data <= 5 => Ok(Priority(data)),
            _ => Err(form_value),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Todo {
    id: ID,
    priority: Priority,
    title: String,
}

type TodoRepository = Mutex<HashMap<ID, Todo>>;

#[get("/", format = "json")]
fn index(todos: State<TodoRepository>) -> JsonValue {
    let hashmap = todos.lock().unwrap();
    let todos_map = hashmap.deref();
    let mut data: Vec<&Todo> = Vec::new();

    for (_, v) in todos_map {
        data.push(v)
    }
    json!(data)
}

#[get("/<id>", format = "json")]
fn get_single_todo(id: ID, todos: State<TodoRepository>) -> Option<Json<Todo>> {
    let hashmap = todos.lock().expect("map locked");
    hashmap.get(&id).map(|content| {
        Json(Todo {
            id: content.id.clone(),
            title: content.title.clone(),
            priority: content.priority,
        })
    })
}

#[post("/", format = "json", data = "<todo>")]
fn add_todo(todo: Json<Todo>, todos: State<TodoRepository>) -> JsonValue {
    let mut hashmap = todos.lock().expect("map locked");
    hashmap.insert(todo.0.id, todo.0);
    json!({ "status": "ok" })
}

#[delete("/<id>", format = "json")]
fn delete_todo(id: ID, todos: State<TodoRepository>) -> JsonValue {
    let mut hashmap = todos.lock().expect("map locked");
    hashmap.remove(&id);
    json!({ "status": "ok" })
}

#[put("/<id>", format = "json", data = "<todo>")]
fn update_todo(id: ID, todo: Json<Todo>, todos: State<TodoRepository>) -> Option<JsonValue> {
    let mut hashmap = todos.lock().expect("map locked");
    if hashmap.contains_key(&id) {
        hashmap.insert(id, todo.0);
        Some(json!({ "status": "ok" }))
    } else {
        None
    }
}

#[catch(404)]
fn not_found() -> JsonValue {
    json!({
        "status": "error",
        "reason": "Resource was not found."
    })
}

fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .register(catchers![not_found])
        .mount(
            "/",
            routes![index, get_single_todo, add_todo, delete_todo, update_todo],
        )
        .manage(Mutex::new(HashMap::<ID, Todo>::new()))
}

fn main() {
    rocket().launch();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::http::{ContentType, Status};
    use rocket::local::Client;

    #[test]
    fn bad_get_put() {
        let client = Client::new(rocket()).unwrap();

        // Try to get a message with an ID that doesn't exist.
        let mut res = client.get("/99").header(ContentType::JSON).dispatch();
        assert_eq!(res.status(), Status::NotFound);

        let body = res.body_string().unwrap();
        assert!(body.contains("error"));
        assert!(body.contains("Resource was not found."));

        // Try to get a message with an invalid ID.
        let mut res = client.get("/hi").header(ContentType::JSON).dispatch();
        let body = res.body_string().unwrap();
        assert_eq!(res.status(), Status::NotFound);
        assert!(body.contains("error"));

        // Try to put a message without a proper body.
        let res = client.put("/80").header(ContentType::JSON).dispatch();
        assert_eq!(res.status(), Status::BadRequest);

        // Try to put a message for an ID that doesn't exist.
        let res = client
            .put("/80")
            .header(ContentType::JSON)
            .body(r#"{ "id": 80, "title": "todo-1", "priority": 4 }"#)
            .dispatch();

        assert_eq!(res.status(), Status::NotFound);
    }

    #[test]
    fn post_get_put_get() {
        let client = Client::new(rocket()).unwrap();

        // Check that no todo exist at default
        let mut res = client.get("/").header(ContentType::JSON).dispatch();
        assert_eq!(res.status(), Status::Ok);
        let body = res.body().unwrap().into_string().unwrap();
        assert_eq!(body, "[]");
        // Check that a todo with ID 1 doesn't exist.
        let res = client.get("/1").header(ContentType::JSON).dispatch();
        assert_eq!(res.status(), Status::NotFound);

        // Add a new todo with ID 1.
        let res = client
            .post("/")
            .header(ContentType::JSON)
            .body(r#"{ "id": 1, "title": "write tests", "priority": 4 }"#)
            .dispatch();

        assert_eq!(res.status(), Status::Ok);

        // Check that the todo exists with the correct contents.
        let mut res = client.get("/1").header(ContentType::JSON).dispatch();
        assert_eq!(res.status(), Status::Ok);
        let body = res.body().unwrap().into_string().unwrap();
        assert!(body.contains("write tests"));

        // Change the todo.
        let res = client
            .put("/1")
            .header(ContentType::JSON)
            .body(r#"{ "id": 1, "title": "write tests updated", "priority": 3 }"#)
            .dispatch();

        assert_eq!(res.status(), Status::Ok);

        // Check that the todo exists with the updated contents.
        let mut res = client.get("/1").header(ContentType::JSON).dispatch();
        assert_eq!(res.status(), Status::Ok);
        let body = res.body().unwrap().into_string().unwrap();
        assert!(!body.contains("Hello, world!"));
        assert!(body.contains("write tests updated"));
    }
}
