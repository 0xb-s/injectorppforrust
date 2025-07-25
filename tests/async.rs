use injectorpp::interface::injector::*;

async fn simple_async_func_u32_add_one(x: u32) -> u32 {
    x + 1
}

async fn simple_async_func_u32_add_two(x: u32) -> u32 {
    x + 2
}

async fn simple_async_func_bool(x: bool) -> bool {
    x
}

struct HttpClientTest {
    pub url: String,
}

impl HttpClientTest {
    pub async fn get(&self) -> String {
        format!("GET {}", self.url)
    }

    pub async fn post(&self, payload: &str) -> String {
        format!("POST {} to {}", payload, self.url)
    }
}

#[tokio::test]
async fn test_simple_async_func_should_success() {
    let mut injector = InjectorPP::new();

    injector
        .when_called_async(injectorpp::async_func!(
            simple_async_func_u32_add_one(u32::default()),
            u32
        ))
        .will_return_async(injectorpp::async_return!(123, u32));

    let x = simple_async_func_u32_add_one(1).await;
    assert_eq!(x, 123);

    // simple_async_func_u32_add_two should not be affected
    let x = simple_async_func_u32_add_two(1).await;
    assert_eq!(x, 3);

    injector
        .when_called_async(injectorpp::async_func!(
            simple_async_func_u32_add_two(u32::default()),
            u32
        ))
        .will_return_async(injectorpp::async_return!(678, u32));

    // Now because it's faked the return value should be changed
    let x = simple_async_func_u32_add_two(1).await;
    assert_eq!(x, 678);

    // simple_async_func_bool should not be affected
    let y = simple_async_func_bool(true).await;
    assert_eq!(y, true);

    injector
        .when_called_async(injectorpp::async_func!(
            simple_async_func_bool(bool::default()),
            bool
        ))
        .will_return_async(injectorpp::async_return!(false, bool));

    // Now because it's faked the return value should be false
    let y = simple_async_func_bool(true).await;
    assert_eq!(y, false);
}

#[tokio::test]
async fn test_complex_struct_async_func_without_param_should_success() {
    {
        // This is a temporary instance that is needed for async function fake.
        // Parameter does not matter.
        let temp_client = HttpClientTest {
            url: String::default(),
        };

        let mut injector = InjectorPP::new();
        injector
            .when_called_async(injectorpp::async_func!(temp_client.get(), String))
            .will_return_async(injectorpp::async_return!(
                "Fake GET response".to_string(),
                String
            ));

        // Now the real client will be used and its behavior should be faked
        let real_client = HttpClientTest {
            url: "https://test.com".to_string(),
        };

        let result = real_client.get().await;
        assert_eq!(result, "Fake GET response".to_string());
    }

    let real_client = HttpClientTest {
        url: "https://test.com".to_string(),
    };

    // The original function should be called as the injector is out of scope
    let result = real_client.get().await;
    assert_eq!(result, "GET https://test.com".to_string());
}

#[tokio::test]
async fn test_complex_struct_async_func_with_param_should_success() {
    {
        // This is a temporary instance that is needed for async function fake.
        // Parameter does not matter.
        let temp_client = HttpClientTest {
            url: String::default(),
        };

        let mut injector = InjectorPP::new();
        injector
            .when_called_async(injectorpp::async_func!(
                temp_client.post("test payload"),
                String
            ))
            .will_return_async(injectorpp::async_return!(
                "Fake POST response".to_string(),
                String
            ));

        // Now the real client will be used and its behavior should be faked
        let real_client = HttpClientTest {
            url: "https://test.com".to_string(),
        };

        let result = real_client.post("test payload").await;
        assert_eq!(result, "Fake POST response".to_string());
    }

    let real_client = HttpClientTest {
        url: "https://test.com".to_string(),
    };

    // The original function should be called as the injector is out of scope
    let result = real_client.post("test payload").await;
    assert_eq!(result, "POST test payload to https://test.com".to_string());
}
