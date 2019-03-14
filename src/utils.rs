#[allow(non_snake_case)]
pub fn P<T>(x: T) -> Box<T> {
    Box::new(x)
}
