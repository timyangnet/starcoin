// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::cli_state::CliState;
use crate::StarcoinOpt;
use anyhow::{format_err, Result};
use scmd::{CommandAction, ExecContext};
use starcoin_crypto::hash::HashValue;
use starcoin_rpc_api::types::FunctionIdView;
use starcoin_rpc_client::RemoteStateReader;
use starcoin_state_api::AccountStateReader;
use starcoin_types::transaction::{
    parse_transaction_argument, RawUserTransaction, TransactionArgument,
};
use starcoin_vm_types::account_address::AccountAddress;
use starcoin_vm_types::transaction::ScriptFunction;
use starcoin_vm_types::transaction_argument::convert_txn_args;
use starcoin_vm_types::{language_storage::TypeTag, parser::parse_type_tag};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "execute-function")]
pub struct ExecuteScriptFunctionOpt {
    #[structopt(short = "s")]
    /// if `sender` is absent, use default account.
    sender: Option<AccountAddress>,

    #[structopt(long = "function", name = "script-function")]
    /// script function to execute, example: 0x1::TransferScripts::peer_to_peer
    script_function: FunctionIdView,

    #[structopt(
    short = "t",
    long = "type_tag",
    name = "type-tag",
    parse(try_from_str = parse_type_tag)
    )]
    /// type tags for the script
    type_tags: Option<Vec<TypeTag>>,

    #[structopt(long = "arg", name = "transaction-args", parse(try_from_str = parse_transaction_argument))]
    /// args for the script.
    args: Option<Vec<TransactionArgument>>,

    #[structopt(
        name = "expiration_time",
        long = "timeout",
        default_value = "3000",
        help = "how long(in seconds) the txn stay alive"
    )]
    expiration_time: u64,

    #[structopt(
        short = "g",
        name = "max-gas-amount",
        default_value = "10000000",
        help = "max gas used to deploy the module"
    )]
    max_gas_amount: u64,
    #[structopt(
        short = "p",
        long = "gas-price",
        name = "price of gas",
        default_value = "1",
        help = "gas price used to deploy the module"
    )]
    gas_price: u64,

    #[structopt(
        short = "b",
        name = "blocking-mode",
        long = "blocking",
        help = "blocking wait txn mined"
    )]
    blocking: bool,
}

pub struct ExecuteScriptFunctionCmd;

impl CommandAction for ExecuteScriptFunctionCmd {
    type State = CliState;
    type GlobalOpt = StarcoinOpt;
    type Opt = ExecuteScriptFunctionOpt;
    type ReturnItem = HashValue;

    fn run(
        &self,
        ctx: &ExecContext<Self::State, Self::GlobalOpt, Self::Opt>,
    ) -> Result<Self::ReturnItem> {
        let opt = ctx.opt();
        let client = ctx.state().client();
        let node_info = client.node_info()?;

        let sender = ctx.state().get_account_or_default(opt.sender)?;
        let chain_state_reader = RemoteStateReader::new(client)?;
        let account_state_reader = AccountStateReader::new(&chain_state_reader);
        let account_resource = account_state_reader.get_account_resource(&sender.address)?;
        let account_resource = account_resource.ok_or_else(|| {
            format_err!("account of address {} not exists on chain", sender.address)
        })?;
        let expiration_time = opt.expiration_time + node_info.now_seconds;

        let type_tags = opt.type_tags.clone().unwrap_or_default();
        let args = opt.args.clone().unwrap_or_default();
        let script_function = opt.script_function.clone().0;
        let script_txn = RawUserTransaction::new_script_function(
            sender.address,
            account_resource.sequence_number(),
            ScriptFunction::new(
                script_function.module,
                script_function.function,
                type_tags,
                convert_txn_args(&args),
            ),
            opt.max_gas_amount,
            opt.gas_price,
            expiration_time,
            ctx.state().net().chain_id(),
        );

        let signed_txn = client.account_sign_txn(script_txn)?;
        let txn_hash = signed_txn.id();
        client.submit_transaction(signed_txn)?;
        println!("txn {:#x} submitted.", txn_hash);

        if opt.blocking {
            ctx.state().watch_txn(txn_hash)?;
        }

        Ok(txn_hash)
    }
}
