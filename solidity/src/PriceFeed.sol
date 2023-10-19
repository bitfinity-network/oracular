// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import ""


contract PriceFeedSinglePair is AggregatorV3Interface {
    address public owner;
    string public pairDescription;
    uint8 public pairDecimals;
    uint256 public pairVersion;
    uint256 public currentRoundId;

    struct RoundData {
        uint256 answer;
        uint256 timestamp;
    }

    mapping(uint256 => RoundData) public rounds;

    modifier onlyOwner() {
        require(
            msg.sender == owner,
            "Only the contract owner can call this function"
        );
        _;
    }

    constructor(string memory _description, uint8 _decimals, uint256 _version) {
        owner = msg.sender;
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
        returns (uint256 roundId, uint256 answer, uint256 timestamp)
    {
        require(_roundId <= currentRoundId, "Round not available");
        return (_roundId, rounds[_roundId].answer, rounds[_roundId].timestamp);
    }

    function latestRoundData()
        external
        view
        override
        returns (uint256 roundId, uint256 answer, uint256 timestamp)
    {
        return (
            currentRoundId,
            rounds[currentRoundId].answer,
            rounds[currentRoundId].timestamp
        );
    }

    event AnswerUpdated(
        uint256 indexed answer,
        uint256 indexed roundId,
        uint256 updatedAt
    );
}
