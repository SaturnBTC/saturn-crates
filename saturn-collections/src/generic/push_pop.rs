pub trait PushPopCollection<T> {
    fn push(&mut self, item: T);
    fn pop(&mut self) -> Option<T>;
    fn as_slice(&self) -> &[T];
    fn len(&self) -> usize;
}

// Implement for Vec
impl<T> PushPopCollection<T> for Vec<T> {
    fn push(&mut self, item: T) {
        self.push(item);
    }

    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    fn as_slice(&self) -> &[T] {
        self.as_slice()
    }

    fn len(&self) -> usize {
        self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_len() {
        let mut collection: Vec<i32> = Vec::new();
        collection.push(1);
        collection.push(2);
        assert_eq!(collection.len(), 2);
    }

    #[test]
    fn test_pop() {
        let mut collection: Vec<i32> = vec![10, 20];
        let item = collection.pop();
        assert_eq!(item, Some(20));
        assert_eq!(collection.len(), 1);
    }

    #[test]
    fn test_pop_empty() {
        let mut collection: Vec<i32> = Vec::new();
        assert_eq!(collection.pop(), None);
    }

    #[test]
    fn test_as_slice() {
        let mut collection: Vec<i32> = Vec::new();
        collection.push(5);
        collection.push(10);
        collection.push(15);

        let slice = collection.as_slice();
        assert_eq!(slice, &[5, 10, 15]);
    }

    #[test]
    fn test_combined_operations() {
        let mut collection: Vec<i32> = Vec::new();
        collection.push(100);
        collection.push(200);
        collection.pop();
        collection.push(300);

        let slice = collection.as_slice();
        assert_eq!(slice, &[100, 300]);
        assert_eq!(collection.len(), 2);
    }
}
