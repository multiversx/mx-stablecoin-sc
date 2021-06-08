# sc-stablecoin-rs

## Introduction

Stablecoins are a type of cryptocurrency that aims to have a 1:1 exchange rate between 1 crypto coin and 1 fiat equivalent (for example, having one coin have the same value as one dollar). As their name suggests, stablecoins aim to provide stability in the cryptocurrency world. Cryptocurrencies like Bitcoin can’t really be used as a day to day currency, as they're way too volatile. Fluctuations of 1-2k on a daily basis are just not acceptable for a currency.  

There are two main types of stablecoins:
- algorithmic
- collateralized

We're going to focus on the latter one in this document. This type of stablecoin is backed up by some item of equal value, meaning that you can always exchange back and forth with (almost) no price fluctuation. There are three main types of items being used as collateral:
- Fiat currency (US Dollar)
- Commodities (Interchangeable assets like jewelry or gold)
- Other crypto coins

In this case, we're focusing on the third one: cryptocoin-based collateral.  

## Overcollateralized stablecoins

We've talked a bit about collateralized stablecoins, but unfortunately, they've got a problem: it's very easy to get "free money" out of them. Their core mechanic is that you sell some sort of asset for its value in dollars at that point in time, and you can re-buy your asset back at a later time.  

But what if the asset's value decreases? Even with the imposed debt fee, the stablecoin ecosystem might not be able to cover up its losses. This would lead to a gradual increase in total supply of stablecoins, which would make its value decrease below $1. This is especially a problem for cryptocoins as collateral.  

To try and counteract this problem, the concept of overcollateralized stablecoin was created: instead of giving the user the full value of their asset in dollars at that point in time, instead we only give them part of its value, for example, 75% of the value. This discourages trading back and forth for a profit, but instead promotes the idea of using the stablecoin for its intended purpose of being a store of value and medium of exchange.  

## Buying stablecoins

Buying stablecoins (also known as "borrowing") is done through the "liquidity pool" smart contract, through the `borrow` endpoint:  

```
#[payable("*")]
#[endpoint]
fn borrow(
    &self,
    #[payment_token] collateral_id: TokenIdentifier,
    #[payment] collateral_amount: Self::BigUint,
) -> SCResult<()>
```

It requires no arguments, just the transfer of the appropriate collateral. A so-called "debt position" will be created inside the smart contract for this entry. You will receive stablecoins according to the following formula `collateral_value_in_dollars * borrow_rate`. For example, if you deposit 50 eGLD, with eGLD being valued at $100, and a borrow rate of 0.5, you will receive 2500 stablecoins ($2500).  

You will also receive "debt tokens", which are some semi-fungible tokens that allow you to repay your debt and get your eGLD back. You will receive an amount equal to the eGLD transferred, so in the example above, you will receive 50 * 10^18 SFTs. This makes it so you can reclaim partial amounts or even sell your SFTs to someone else and let them buy your locked eGLD instead.  

## Rebuying your assets

This is a two-step process. First, you have to lock an amount of "debt tokens" equal to the amount of tokens you want to reclaim. This is done through the `lockDebtTokens` endpoint:

```
#[payable("*")]
#[endpoint(lockDebtTokens)]
fn lock_debt_tokens(
    &self,
    #[payment_token] debt_token: TokenIdentifier,
    #[payment] amount: Self::BigUint,
) -> SCResult<u64>
```

Then, you have to pay the value in dollars of the assets you want to reclaim, plus a debt, calculated as follows: `time_unit_percentage * borrow_rate * amount`, where:
- `time_unit_percentage` is how many seconds have passed since you borrowed the stablecoins, expressed as a percentage of seconds in a year, so if you've waited a year (31,556,926 seconds), this percentage would be 100%.  
- `borrow_rate` is the same borrow rate percentage used at the time of borrowing (50% in the provided example).  
- `amount` is the number of tokens you want to repay

Adding to the example from the borrowing section, to repay the 50 eGLD at $100 dollar price per eGLD after 1 year, you'd have to pay (50 eGLD * $100) * (1 + 1 year * 0.5 borrow rate) = $7500  

Repaying the debt is done through the `repay` endpoint:  

```
#[payable("*")]
#[endpoint]
fn repay(
    &self,
    position_id: u64,
    #[payment_token] token_id: TokenIdentifier,
    #[payment] amount: Self::BigUint,
) -> SCResult<()>
```

`position_id` is your SFT's nonce. It's also the value returned by the `lockDebtTokens` endpoint, so you see it there.  

Repaying can only be done with stablecoins, so any other token will be rejected. Don't worry about paying the exact amount, any extra will be returned to you automatically. Also, if you prefer, you can also do partial repays. Keep in mind though that the debt will still keep increasing until you've fully repaid.  

## Liquidation

Liquidation is the process through which assets that are not repaid by their original owner can be bought by someone else. Every "position" in the smart contract has a so-called "health factor", which is a function of time passed and price fluctuation of the asset itself (TBD - exact function not decided yet).  

This is done so the contract is not stuck with locked assets forever if they're not reclaimed. Liquidation is done through the following endpoint:

```
#[payable("*")]
#[endpoint]
fn liquidate(
    &self,
    position_id: u64,
    #[payment_token] token_id: TokenIdentifier,
    #[payment] amount: Self::BigUint,
) -> SCResult<()>
```

Liquidation still requires the buyer to pay the full accumulated debt. The difference is that this process requires no debt tokens, and there may be no partial rebuys.  

## Conclusion

And that's about it. The contract also provides various view functions that help you calculate debt amounts and such so you can see exactly what values you can expect. For more details, feel free to take a look at the implementation. 
