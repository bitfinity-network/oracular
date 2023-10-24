use ethers::contract::abigen;

abigen!(
    AggregatorV2V3InterfaceApi,
    "$OUT_DIR/AggregatorV2V3Interface.sol/AggregatorV2V3Interface.json"
);

abigen!(PriceFeedApi, "$OUT_DIR/PriceFeed.sol/PriceFeed.json",);
