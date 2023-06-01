public class AIPlayer : Player
{
    public AIPlayer(string name, char symbol) : base(name, symbol) { } // Konstruktor klasy AIPlayer, wywo�uje konstruktor bazowy z klasy Player

    public override void MakeMove(Board board) // Implementacja metody MakeMove z klasy bazowej Player
    {
        Random random = new Random(); // Inicjalizacja nowego obiektu klasy Random
        int x, y;

        do
        {
            x = random.Next(0, 3); // Losowanie liczby ca�kowitej z zakresu [0, 3)
            y = random.Next(0, 3); // Losowanie liczby ca�kowitej z zakresu [0, 3)
        } while (!board.PlaceSymbol(x, y, Symbol)); // Wykonuj losowanie, dop�ki nie uda si� umie�ci� symbolu AIPlayer na planszy
    }
}
