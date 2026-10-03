use otelo_journal::open_sync_channel;

#[tokio::test]
async fn ticket_waits_until_its_frame_is_synced() {
    let (publisher, subscription) = open_sync_channel();
    publisher.publish_synced_frames(1);
    let waiting = tokio::spawn(subscription.ticket_for_frame(2).wait_until_synced());
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());

    publisher.publish_synced_frames(2);

    waiting.await.unwrap().unwrap();
}

#[tokio::test]
async fn ticket_of_a_frame_synced_before_a_failure_succeeds() {
    let (publisher, subscription) = open_sync_channel();
    publisher.publish_synced_frames(3);

    publisher.publish_failure("the disk is gone");

    subscription
        .ticket_for_frame(3)
        .wait_until_synced()
        .await
        .unwrap();
    let error = subscription
        .ticket_for_frame(4)
        .wait_until_synced()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("the disk is gone"), "{error}");
}

#[tokio::test]
async fn ticket_fails_when_the_journal_stops_before_the_sync() {
    let (publisher, subscription) = open_sync_channel();
    let ticket = subscription.ticket_for_frame(1);

    drop(publisher);

    assert!(ticket.wait_until_synced().await.is_err());
}

#[tokio::test]
async fn synced_frames_never_go_back() {
    let (publisher, subscription) = open_sync_channel();
    publisher.publish_synced_frames(5);

    publisher.publish_synced_frames(2);

    subscription
        .ticket_for_frame(5)
        .wait_until_synced()
        .await
        .unwrap();
}
