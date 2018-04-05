pub trait Coalesce<T> {
    fn coalesce(self) -> Vec<T>;
}

impl<T> Coalesce<T> for (Vec<T>, T) {
    fn coalesce(self) -> Vec<T> {
        let (mut xs, x) = self;
        xs.push(x);
        xs
    }
}
