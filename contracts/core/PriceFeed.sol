// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "../interfaces/AggregatorV2V3Interface.sol";
import "../Ownable.sol";

contract PriceFeed is AggregatorV2V3Interface, Ownable {
    // The description of the pair
    string public pairDescription;
    uint8 public pairDecimals;
    uint256 public pairVersion;
    uint256 public currentRoundId;

    // RoundData contains the answer and the timestamp of the price feed update
    struct RoundData {
        uint256 answer;
        uint256 timestamp;
    }

    // Mapping of roundId to RoundData
    mapping(uint256 => RoundData) public rounds;

    constructor(
        string memory _description,
        uint8 _decimals,
        uint256 _version
    ) Ownable(msg.sender) {
        pairDescription = _description;
        pairDecimals = _decimals;
        pairVersion = _version;
        currentRoundId = 0;
    }

    function updatePrice(uint256 _price) external onlyOwner {
        currentRoundId += 1;
        rounds[currentRoundId] = RoundData({
            answer: _price,
            timestamp: block.timestamp
        });

        // Emitting the event for the update
        emit AnswerUpdated(_price, currentRoundId, block.timestamp);
    }

    function decimals() external view override returns (uint8) {
        return pairDecimals;
    }

    function description() external view override returns (string memory) {
        return pairDescription;
    }

    function version() external view override returns (uint256) {
        return pairVersion;
    }

    function getRoundData(
        uint80 _roundId
    )
        external
        view
        override
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        )
    {
        RoundData storage round = rounds[_roundId];
        return (
            _roundId,
            int256(round.answer),
            round.timestamp,
            round.timestamp,
            _roundId
        );
    }

    function latestRoundData()
        external
        view
        override
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        )
    {
        RoundData storage round = rounds[currentRoundId];
        return (
            uint80(currentRoundId),
            int256(round.answer),
            round.timestamp,
            round.timestamp,
            uint80(currentRoundId)
        );
    }

    function latestAnswer() external view override returns (int256) {
        return int256(rounds[currentRoundId].answer);
    }

    function latestTimestamp() external view override returns (uint256) {
        return rounds[currentRoundId].timestamp;
    }

    function latestRound() external view override returns (uint256) {
        return currentRoundId;
    }

    function getAnswer(
        uint256 _roundId
    ) external view override returns (int256) {
        return int256(rounds[_roundId].answer);
    }

    function getTimestamp(
        uint256 _roundId
    ) external view override returns (uint256) {
        return rounds[_roundId].timestamp;
    }

    event AnswerUpdated(
        uint256 indexed answer,
        uint256 indexed roundId,
        uint256 updatedAt
    );
}
