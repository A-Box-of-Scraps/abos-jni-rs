public class DefaultType {
    public DefaultPeer peer;

    public DefaultType() {}

    public DefaultType echo(DefaultType other) {
        return other;
    }

    public DefaultPeer echoPeer(DefaultPeer other) {
        return other;
    }

    public DefaultPeer[] echoPeers(DefaultPeer[] others) {
        return others;
    }

    public DefaultType[][] echoMatrix(DefaultType[][] others) {
        return others;
    }

    public Child echoChild(Child child) {
        return child;
    }

    public static class Child extends DefaultType {
        public DefaultType parent() {
            return this;
        }
    }
}
