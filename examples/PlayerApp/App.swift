import SwiftUI
import fsm_example

@main
struct PlayerApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    @State private var state: PlayerState = .Standing
    @State private var score: Int32 = 0
    @State private var history: [String] = []

    var body: some View {
        VStack(spacing: 20) {
            Text("Rust-Powered Swift App")
                .font(.largeTitle)
                .bold()

            Text("Compiled directly from Rust to Native Swift (Zero FFI)")
                .font(.subheadline)
                .foregroundColor(.secondary)

            Divider()

            HStack {
                Text("Current Player State:")
                Text(state.rawValue)
                    .font(.title2)
                    .bold()
                    .foregroundColor(colorForState(state))
            }

            Text("Score: \(score)")
                .font(.headline)

            HStack(spacing: 12) {
                Button("Move") { dispatch(.Move) }
                Button("Jump") { dispatch(.Jump) }
                Button("Land") { dispatch(.Land) }
                Button("Stop") { dispatch(.Stop) }
            }
            .buttonStyle(.borderedProminent)

            Divider()

            VStack(alignment: .leading) {
                Text("State Transition Log:")
                    .font(.caption)
                    .foregroundColor(.secondary)
                ScrollView {
                    ForEach(history.indices.reversed(), id: \.self) { idx in
                        Text(history[idx])
                            .font(.system(.caption, design: .monospaced))
                    }
                }
                .frame(height: 120)
            }
        }
        .padding(30)
        .frame(width: 440, height: 400)
    }

    private func dispatch(_ action: PlayerAction) {
        let previous = state
        state = update_player(state, action)
        score = calculate_score(score, 1, 10)
        history.append("\(previous.rawValue) + \(action.rawValue) -> \(state.rawValue) (score: \(score))")
    }

    private func colorForState(_ s: PlayerState) -> Color {
        switch s {
        case .Standing: return .blue
        case .Running: return .green
        case .Jumping: return .orange
        case .Falling: return .red
        }
    }
}
