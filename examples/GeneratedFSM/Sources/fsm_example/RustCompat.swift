// Minimal Swift compatibility shims for rust-swift compiler
import Foundation

public struct RustCompat {
    public static func saturatingAdd<T: FixedWidthInteger>(_ a: T, _ b: T) -> T {
        let (res, overflow) = a.addingReportingOverflow(b)
        if overflow {
            return b >= 0 ? T.max : T.min
        }
        return res
    }

    public static func saturatingSub<T: FixedWidthInteger>(_ a: T, _ b: T) -> T {
        let (res, overflow) = a.subtractingReportingOverflow(b)
        if overflow {
            return b >= 0 ? T.min : T.max
        }
        return res
    }
}
