"""MGN section 6: outcomes are FIFO within an event key, not globally."""


def outcome_key(outcome: dict) -> tuple:
    return (outcome['kind'], outcome.get('seat'),
            *(tuple(sorted(outcome[field])) if field in outcome else None
              for field in ('objects', 'from', 'pile')))


def matches_request(outcome: dict, request: dict) -> bool:
    """A request may omit the set when an adapter prefetches a transition.

    The execution owner must still authenticate that set before consuming it.
    """
    return (outcome['kind'] == request['kind']
            and all(outcome.get(field) == request[field] for field in ('seat',) if field in request)
            and all(set(outcome.get(field, [])) == set(request[field])
                    for field in ('objects', 'from', 'pile') if field in request))
