public class AIPlayer : Player
{
    public AIPlayer(string name, char symbol) : base(name, symbol) { } // Konstruktor klasy AIPlayer, wywo³uje konstruktor bazowy z klasy Player

    public override void MakeMove(Board board) // Implementacja metody MakeMove z klasy bazowej Player
    {
        Random random = new Random(); // Inicjalizacja nowego obiektu klasy Random
        int x, y;

        do
        {
            x = random.Next(0, 3); // Losowanie liczby ca³kowitej z zakresu [0, 3)
            y = random.Next(0, 3); // Losowanie liczby ca³kowitej z zakresu [0, 3)
        } while (!board.PlaceSymbol(x, y, Symbol)); // Wykonuj losowanie, dopóki nie uda siê umieœciæ symbolu AIPlayer na planszy
    }
}
