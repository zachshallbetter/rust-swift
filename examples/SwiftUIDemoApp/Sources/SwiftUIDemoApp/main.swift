import SwiftUI
import Observation

@main
struct SwiftUIDemoApp: App {
    @State private var model = CounterModel(count: 0, title: "Counter")

    var body: some Scene {
        WindowGroup {
            CounterView(model: model)
                .frame(width: 360, height: 200)
                .padding()
        }
    }
}
