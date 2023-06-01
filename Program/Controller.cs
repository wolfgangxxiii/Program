using System;
using System.IO;

namespace TwojaPrzestrzenNazw
{
    public static class Controller
    {
        // Metoda rozpoczynaj�ca dzia�anie gry
        public static void StartGame()
        {
            while (true)
            {
                Console.Clear();
                Console.WriteLine("K�ko i krzy�yk");
                Console.WriteLine("1. Gra dla dw�ch graczy");
                Console.WriteLine("2. Gra z automatem");
                Console.WriteLine("3. Wy�wietl wyniki");
                Console.WriteLine("4. Wyj�cie");
                Console.Write("Wybierz opcj� (1-4): ");

                int option = Convert.ToInt32(Console.ReadLine());

                switch (option)
                {
                    case 1:
                        Console.WriteLine("Podaj imi� gracza 1:");
                        string playerName1 = Console.ReadLine();
                        Player player1 = new HumanPlayer(playerName1, 'X');

                        Console.WriteLine("Podaj imi� gracza 2:");
                        string playerName2 = Console.ReadLine();
                        Player player2 = new HumanPlayer(playerName2, 'O');

                        PlayGame(player1, player2); // Wywo�anie metody rozpoczynaj�cej gr� dla dw�ch graczy
                        break;

                    case 2:
                        Console.WriteLine("Podaj imi� gracza:");
                        string playerName = Console.ReadLine();
                        Player humanPlayer = new HumanPlayer(playerName, 'X');

                        Player aiPlayer = new AIPlayer("Komputer", 'O');
                        PlayGame(humanPlayer, aiPlayer); // Wywo�anie metody rozpoczynaj�cej gr� z graczem AI
                        break;

                    case 3:
                        DisplayResults(); // Wywo�anie metody wy�wietlaj�cej wyniki
                        Console.ReadKey();
                        break;

                    case 4:
                        return;

                    default:
                        Console.WriteLine("Niepoprawna opcja. Spr�buj ponownie.");
                        Console.ReadKey();
                        break;
                }
            }
        }

        // Metoda rozpoczynaj�ca gr� dla dw�ch graczy
        static void PlayGame(Player player1, Player player2)
        {
            Board board = new Board();
            int currentPlayer = 1;
            int movesMade = 0;
            bool gameOver = false;

            while (!gameOver)
            {
                Console.Clear();
                board.Display();
                Console.WriteLine($"Ruch gracza: {currentPlayer}");
                Console.WriteLine($"Ruch {movesMade + 1}");

                Player currentPlayerObject = currentPlayer == 1 ? player1 : player2;
                currentPlayerObject.MakeMove(board); // Wywo�anie metody wykonuj�cej ruch gracza

                movesMade++;

                if (board.CheckForWinner(currentPlayerObject.Symbol))
                {
                    gameOver = true;
                    Console.Clear();
                    board.Display();
                    Console.WriteLine($"Gracz {currentPlayer} wygrywa!");
                    SaveResult(player1.Name, player2.Name, currentPlayerObject.Name); // Wywo�anie metody zapisuj�cej wynik
                }
                else if (movesMade == 9)
                {
                    gameOver = true;
                    Console.Clear();
                    board.Display();
                    Console.WriteLine("Remis!");
                    SaveResult(player1.Name, player2.Name, "Remis"); // Wywo�anie metody zapisuj�cej wynik remisu
                }
                else
                {
                    currentPlayer = currentPlayer == 1 ? 2 : 1; // Zmiana kolejno�ci graczy
                }
            }

            Console.WriteLine("Naci�nij dowolny klawisz, aby kontynuowa�...");
            Console.ReadKey();
        }

        // Metoda zapisuj�ca wynik gry
        static void SaveResult(string player1Name, string player2Name, string winner)
        {
            string result = $"{DateTime.Now} - {player1Name} vs {player2Name} - Wygrany: {winner}";
            File.AppendAllText("results.txt", result + Environment.NewLine);
        }

        // Metoda wy�wietlaj�ca zapisane wyniki
        static void DisplayResults()
        {
            Console.Clear();
            Console.WriteLine("Wyniki:");
            Console.WriteLine();

            if (File.Exists("results.txt"))
            {
                string[] results = File.ReadAllLines("results.txt");

                foreach (string result in results)
                {
                    Console.WriteLine(result);
                }
            }
            else
            {
                Console.WriteLine("Brak zapisanych wynik�w.");
            }

            Console.WriteLine();
            Console.WriteLine("Naci�nij dowolny klawisz, aby wr�ci� do menu...");
        }
    }
}
