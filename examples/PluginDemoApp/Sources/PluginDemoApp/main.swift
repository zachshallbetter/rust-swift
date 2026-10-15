import Foundation

let initial = MachineStatus.Starting
let next = advance(initial)
print("PLUGIN_RESULT:\(initial.rawValue)->\(next.rawValue)")
