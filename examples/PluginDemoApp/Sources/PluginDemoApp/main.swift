import Foundation

@main
struct Main {
    static func main() {
        let initial = MachineStatus.Starting
        let next = advance(initial)
        print("PLUGIN_RESULT:\(initial.rawValue)->\(next.rawValue)")
    }
}
