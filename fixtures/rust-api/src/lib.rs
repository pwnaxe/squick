use serde::Serialize;

/// A book in the catalog.
#[derive(Serialize)]
pub struct Book {
    pub id: u32,
    pub title: String,
}

pub trait Repository {
    fn find(&self, id: u32) -> Option<Book>;
}

pub struct InMemoryRepository {
    books: Vec<Book>,
}

impl Repository for InMemoryRepository {
    fn find(&self, id: u32) -> Option<Book> {
        for book in &self.books {
            if book.id == id {
                return Some(Book {
                    id: book.id,
                    title: book.title.clone(),
                });
            }
        }
        None
    }
}

/// Returns a greeting for the given name.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
