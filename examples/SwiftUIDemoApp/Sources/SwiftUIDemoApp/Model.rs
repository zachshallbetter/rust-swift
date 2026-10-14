#[observable]
pub struct CounterModel {
    pub count: i64,
    pub title: String,
}

impl CounterModel {
    pub fn increment(&mut self) {
        self.count = self.count.wrapping_add(1);
    }

    pub fn decrement(&mut self) {
        self.count = self.count.wrapping_sub(1);
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }
}

#[view]
pub struct CounterView {
    pub model: CounterModel,
}

impl CounterView {
    pub fn body(&self) {
        VStack(vec![
            Text("Rust-Powered SwiftUI Demo").font("headline").padding(),
            HStack(vec![
                Button("Decrement", "model.decrement()"),
                Spacer(),
                Button("Increment", "model.increment()"),
            ]),
            Button("Reset", "model.reset()"),
        ])
    }
}
