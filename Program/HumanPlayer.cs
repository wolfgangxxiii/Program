public class HumanPlayer : Player
{
    public HumanPlayer(string name, char symbol) : base(name, symbol) { }

    // Metoda umo¿liwiaj¹ca cz³owiekowi wykonanie ruchu w grze
    public override void MakeMove(Board board)
    {
        Console.Write("\nPodaj numer wiersza (1-3): ");
        int row = Convert.ToInt32(Console.ReadLine()); // Odczytanie numeru wiersza od gracza
        while(row >3 || row < 1)
        {
            Console.WriteLine("\nB³¹d wyboru wiersza!");
            Console.Write("Podaj ponownie  numer (1-3): ");
            row = Convert.ToInt32(Console.ReadLine());
        }

        Console.Write("\nPodaj numer kolumny (1-3): ");
        int col = Convert.ToInt32(Console.ReadLine()); // Odczytanie numeru kolumny od gracza
        while (col > 3 || col < 1)
        {
            Console.WriteLine("\nB³¹d wyboru kolumny!");
            Console.Write("Podaj ponownie  numer (1-3): ");
            col = Convert.ToInt32(Console.ReadLine());
        }

        bool validMove = board.PlaceSymbol(row - 1, col - 1, Symbol); // Wywo³anie metody umieszczaj¹cej symbol gracza na planszy

        if (!validMove)
        {
            Console.WriteLine("Nieprawid³owy ruch! Spróbuj ponownie."); // Wyœwietlenie komunikatu o nieprawid³owym ruchu
            MakeMove(board); // Ponowne wywo³anie metody MakeMove w przypadku nieprawid³owego ruchu
        }
    }
}
