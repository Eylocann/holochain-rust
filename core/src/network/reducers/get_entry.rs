use crate::{
    action::ActionWrapper, context::Context,
    network::{
        reducers::initialized,
        state::NetworkState,
    }
};
use holochain_core_types::{cas::content::Address, error::HolochainError};
use holochain_net_connection::{
    net_connection::NetConnection,
    protocol_wrapper::{GetDhtData, ProtocolWrapper},
};
use std::sync::Arc;

fn inner(network_state: &mut NetworkState, address: &Address) -> Result<(), HolochainError> {
    initialized(network_state)?;

    let data = GetDhtData {
        msg_id: "?".to_string(),
        dna_hash: network_state.dna_hash.clone().unwrap(),
        from_agent_id: network_state.agent_id.clone().unwrap(),
        address: address.to_string(),
    };

    network_state
        .network
        .as_mut()
        .map(|network| {
            network
                .lock()
                .unwrap()
                .send(ProtocolWrapper::GetDht(data).into())
                .map_err(|error| HolochainError::IoError(error.to_string()))
        })
        .expect("Network has to be Some because of check above")
}

pub fn reduce_get_entry(
    _context: Arc<Context>,
    network_state: &mut NetworkState,
    action_wrapper: &ActionWrapper,
) {
    let action = action_wrapper.action();
    let address = unwrap_to!(action => crate::action::Action::GetEntry);

    let result = match inner(network_state, &address) {
        Ok(()) => None,
        Err(err) => Some(Err(err)),
    };

    network_state
        .get_entry_results
        .insert(address.clone(), result);
}

pub fn reduce_get_entry_timeout(
    _context: Arc<Context>,
    network_state: &mut NetworkState,
    action_wrapper: &ActionWrapper,
) {
    let action = action_wrapper.action();
    let address = unwrap_to!(action => crate::action::Action::GetEntryTimeout);

    if network_state.get_entry_results.get(address).is_none() {
        return;
    }

    if network_state
        .get_entry_results
        .get(address)
        .unwrap()
        .is_none()
    {
        network_state
            .get_entry_results
            .insert(address.clone(), Some(Err(HolochainError::Timeout)));
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        action::{Action, ActionWrapper},
        context::mock_network_config,
        instance::tests::test_context,
        state::test_store,
    };
    use holochain_core_types::error::HolochainError;
    use holochain_net_connection::protocol_wrapper::DhtData;

    #[test]
    pub fn reduce_get_entry_without_network_initialized() {
        let context = test_context("alice");
        let store = test_store(context.clone());

        let entry = test_entry();
        let action_wrapper = ActionWrapper::new(Action::GetEntry(entry.address()));

        let store = store.reduce(context.clone(), action_wrapper);
        let maybe_get_entry_result = store
            .network()
            .get_entry_results
            .get(&entry.address())
            .map(|result| result.clone());
        assert_eq!(
            maybe_get_entry_result,
            Some(Some(Err(HolochainError::ErrorGeneric(
                "Network not initialized".to_string()
            ))))
        );
    }

    use holochain_core_types::{cas::content::AddressableContent, entry::test_entry};

    #[test]
    pub fn reduce_get_entry_test() {
        let context = test_context("alice");
        let store = test_store(context.clone());

        let action_wrapper = ActionWrapper::new(Action::InitNetwork((
            mock_network_config(),
            String::from("abcd"),
            String::from("abcd"),
        )));
        let store = store.reduce(context.clone(), action_wrapper);

        let entry = test_entry();
        let action_wrapper = ActionWrapper::new(Action::GetEntry(entry.address()));

        let store = store.reduce(context.clone(), action_wrapper);
        let maybe_get_entry_result = store
            .network()
            .get_entry_results
            .get(&entry.address())
            .map(|result| result.clone());
        assert_eq!(maybe_get_entry_result, Some(None));
    }

    #[test]
    pub fn reduce_get_entry_timeout_test() {
        let context = test_context("alice");
        let store = test_store(context.clone());

        let action_wrapper = ActionWrapper::new(Action::InitNetwork((
            mock_network_config(),
            String::from("abcd"),
            String::from("abcd"),
        )));
        let store = store.reduce(context.clone(), action_wrapper);

        let entry = test_entry();
        let action_wrapper = ActionWrapper::new(Action::GetEntry(entry.address()));

        let store = store.reduce(context.clone(), action_wrapper);
        let maybe_get_entry_result = store
            .network()
            .get_entry_results
            .get(&entry.address())
            .map(|result| result.clone());
        assert_eq!(maybe_get_entry_result, Some(None));

        let action_wrapper = ActionWrapper::new(Action::GetEntryTimeout(entry.address()));
        let store = store.reduce(context.clone(), action_wrapper);
        let maybe_get_entry_result = store
            .network()
            .get_entry_results
            .get(&entry.address())
            .map(|result| result.clone());
        assert_eq!(
            maybe_get_entry_result,
            Some(Some(Err(HolochainError::Timeout)))
        );

        // test that an existing result does not get overwritten by timeout signal
        let dht_data = DhtData {
            msg_id: String::from(""),
            dna_hash: String::from(""),
            agent_id: String::from(""),
            address: entry.address().to_string(),
            content: serde_json::from_str(&serde_json::to_string(&Some(entry.clone())).unwrap())
                .unwrap(),
        };

        let action_wrapper = ActionWrapper::new(Action::HandleGetResult(dht_data));
        let store = store.reduce(context.clone(), action_wrapper);
        let maybe_get_entry_result = store
            .network()
            .get_entry_results
            .get(&entry.address())
            .map(|result| result.clone());
        assert_eq!(maybe_get_entry_result, Some(Some(Ok(Some(entry.clone())))));

        // Ok we got a positive result in the state

        let action_wrapper = ActionWrapper::new(Action::GetEntryTimeout(entry.address()));
        let store = store.reduce(context.clone(), action_wrapper);
        let maybe_get_entry_result = store
            .network()
            .get_entry_results
            .get(&entry.address())
            .map(|result| result.clone());

        // The timeout should not have overwritten the entry
        assert_eq!(maybe_get_entry_result, Some(Some(Ok(Some(entry)))));
    }

}
