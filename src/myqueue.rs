
pub trait MyQueue {
    type ElementType;

    fn push(&self, e : Self::ElementType);
    fn pop(&self) -> Self::ElementType;
}