-- Ship history is not a map's concern, and these two columns only ever fed it: nothing
-- reads `ship_updated_at`, and `ship_item_id` existed to decide when to stamp it. What a
-- pilot is flying is still tracked; how long they have been in that particular hull is not.
alter table character_status
    drop column ship_item_id,
    drop column ship_updated_at;
