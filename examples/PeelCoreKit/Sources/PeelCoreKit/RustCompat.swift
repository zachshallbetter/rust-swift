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

extension String {
    public static func from(_ s: String) -> String {
        return s
    }

    public static func from(_ s: Substring) -> String {
        return String(s)
    }

    public func byteSlice(_ start: Int, _ end: Int) -> String {
        precondition(start >= 0 && end >= start && end <= self.utf8.count, "String slice index out of bounds")
        let startIdx = self.utf8.index(self.utf8.startIndex, offsetBy: start)
        let endIdx = self.utf8.index(self.utf8.startIndex, offsetBy: end)
        guard let sub = String(self.utf8[startIdx..<endIdx]) else {
            fatalError("String slice boundary is not a valid UTF-8 code point")
        }
        return sub
    }

    public func len() -> Int {
        return self.utf8.count
    }

    public func is_empty() -> Bool {
        return self.isEmpty
    }
}

extension Array {
    public func len() -> Int {
        return self.count
    }

    public func is_empty() -> Bool {
        return self.isEmpty
    }
}

extension Optional {
    public func is_some() -> Bool {
        return self != nil
    }

    public func is_none() -> Bool {
        return self == nil
    }

    public func unwrap() -> Wrapped {
        guard let value = self else {
            fatalError("Called unwrap() on a None value")
        }
        return value
    }
}

extension Result {
    public var isOk: Bool {
        switch self {
        case .success: return true
        case .failure: return false
        }
    }

    public var isErr: Bool {
        switch self {
        case .failure: return true
        case .success: return false
        }
    }

    public func is_ok() -> Bool {
        return self.isOk
    }

    public func is_err() -> Bool {
        return self.isErr
    }

    public func unwrap() -> Success {
        switch self {
        case .success(let val): return val
        case .failure(let err): fatalError("Called unwrap() on an Err value: \(err)")
        }
    }
}
