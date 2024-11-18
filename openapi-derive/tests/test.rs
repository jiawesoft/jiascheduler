#[cfg(test)]
mod tests {

    #[test]
    fn test_return_response() {
        struct User {
            #[allow(unused)]
            name: String,
        }
        let _user = User {
            name: "hello world".to_string(),
        };

        // return_response!(User, user);
    }
}
