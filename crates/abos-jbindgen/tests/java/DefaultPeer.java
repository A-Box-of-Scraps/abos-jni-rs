public class DefaultPeer {
    public DefaultType owner;

    public DefaultPeer(DefaultType owner) {
        this.owner = owner;
    }

    public DefaultType echoOwner(DefaultType other) {
        return other;
    }
}
