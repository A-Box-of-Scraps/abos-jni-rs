package com.example;

import java.util.Iterator;

/** Configurable iterator for testing exhaustion and exception handling independently. */
public class TestIterator implements Iterator<Object> {
    private final boolean hasMore;
    public int hasNextCalls;
    public int nextCalls;
    public RuntimeException hasNextFailure;
    public RuntimeException nextFailure;

    public TestIterator(boolean hasMore) {
        this.hasMore = hasMore;
    }

    @Override
    public boolean hasNext() {
        hasNextCalls++;
        if (hasNextFailure != null) {
            throw hasNextFailure;
        }
        return hasMore;
    }

    @Override
    public Object next() {
        nextCalls++;
        if (nextFailure != null) {
            throw nextFailure;
        }
        // Deliberately return an invalid entry when exhausted, like older Android iterators.
        return null;
    }
}
