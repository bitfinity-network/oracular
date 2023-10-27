use ethers::contract::abigen;

abigen!(PriceFeedApi, "$OUT_DIR/PriceFeed.sol/PriceFeed.json");
