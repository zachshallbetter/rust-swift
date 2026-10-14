import XCTest
@testable import fsm_example

final class FsmTests: XCTestCase {
    func testStateTransitions() {
        XCTAssertEqual(update_player(.Standing, .Move), .Running)
        XCTAssertEqual(update_player(.Running, .Stop), .Standing)
        XCTAssertEqual(update_player(.Standing, .Jump), .Jumping)
        XCTAssertEqual(update_player(.Running, .Jump), .Jumping)
        XCTAssertEqual(update_player(.Jumping, .Land), .Standing)
        XCTAssertEqual(update_player(.Jumping, .Stop), .Falling)
        XCTAssertEqual(update_player(.Falling, .Land), .Standing)
        
        // Default / unhandled transitions maintain state
        XCTAssertEqual(update_player(.Falling, .Move), .Falling)
        XCTAssertEqual(update_player(.Standing, .Land), .Standing)
    }

    func testScoreCalculation() {
        XCTAssertEqual(calculate_score(10, 2, 5), 25)
        XCTAssertEqual(calculate_score(100, 3, 50), 350)
        XCTAssertEqual(calculate_score(0, 10, 42), 42)
    }

    func testSimulationSequence() {
        var state = PlayerState.Standing
        var score: Int32 = 0

        let actions: [PlayerAction] = [.Move, .Jump, .Stop, .Land]
        let expectedStates: [PlayerState] = [.Running, .Jumping, .Falling, .Standing]

        for (action, expected) in zip(actions, expectedStates) {
            state = update_player(state, action)
            score = calculate_score(score, 1, 10)
            XCTAssertEqual(state, expected)
        }

        XCTAssertEqual(score, 40)
    }
}
